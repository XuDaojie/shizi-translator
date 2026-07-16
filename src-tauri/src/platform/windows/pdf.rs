//! WinRT `Windows.Data.Pdf`：将 PDF 字节流渲染为第 1 页 `CapturedImage`。

use crate::core::{
    capture::{CapturedImage, CapturedImageFormat},
    ocr::{image_encode::VISION_MAX_LONG_EDGE, OcrError},
};

/// PDF 首页渲染结果：位图 + 文档总页数。
#[derive(Debug, Clone)]
pub struct PdfFirstPage {
    pub image: CapturedImage,
    pub page_count: u32,
}

/// 使用 WinRT 渲染 PDF 第 1 页为 BGRA8 `CapturedImage`。
///
/// 最长边限制与视觉 OCR 一致（`VISION_MAX_LONG_EDGE`）。
/// 禁止 log PDF 全文或像素 dump。
pub async fn render_pdf_first_page(bytes: &[u8]) -> Result<PdfFirstPage, OcrError> {
    let owned = bytes.to_vec();
    tauri::async_runtime::spawn_blocking(move || render_pdf_first_page_blocking(&owned))
        .await
        .map_err(|e| OcrError::PdfRenderFailed(format!("PDF 渲染任务失败: {e}")))?
}

fn render_pdf_first_page_blocking(bytes: &[u8]) -> Result<PdfFirstPage, OcrError> {
    if bytes.is_empty() {
        return Err(OcrError::PdfOpenFailed("PDF 内容为空".into()));
    }
    if !bytes.starts_with(b"%PDF") {
        return Err(OcrError::PdfOpenFailed("不是有效的 PDF 文件".into()));
    }

    render_pdf_first_page_winrt(bytes)
}

fn render_pdf_first_page_winrt(bytes: &[u8]) -> Result<PdfFirstPage, OcrError> {
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform,
        ColorManagementMode, ExifOrientationMode,
    };
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    let input = InMemoryRandomAccessStream::new()
        .map_err(|e| OcrError::PdfOpenFailed(format!("创建输入流失败: {e}")))?;
    {
        let writer = DataWriter::CreateDataWriter(&input)
            .map_err(|e| OcrError::PdfOpenFailed(format!("创建 DataWriter 失败: {e}")))?;
        writer
            .WriteBytes(bytes)
            .map_err(|e| OcrError::PdfOpenFailed(format!("写入 PDF 字节失败: {e}")))?;
        writer
            .StoreAsync()
            .map_err(|e| OcrError::PdfOpenFailed(format!("StoreAsync 失败: {e}")))?
            .get()
            .map_err(|e| OcrError::PdfOpenFailed(format!("StoreAsync 等待失败: {e}")))?;
        writer
            .FlushAsync()
            .map_err(|e| OcrError::PdfOpenFailed(format!("FlushAsync 失败: {e}")))?
            .get()
            .map_err(|e| OcrError::PdfOpenFailed(format!("FlushAsync 等待失败: {e}")))?;
        // Detach 避免 drop writer 时关闭底层 stream
        let _ = writer.DetachStream();
    }
    input
        .Seek(0)
        .map_err(|e| OcrError::PdfOpenFailed(format!("Seek 输入流失败: {e}")))?;

    let doc = PdfDocument::LoadFromStreamAsync(&input)
        .map_err(|e| OcrError::PdfOpenFailed(format!("LoadFromStreamAsync 失败: {e}")))?
        .get()
        .map_err(|e| OcrError::PdfOpenFailed(format!("无法打开 PDF: {e}")))?;

    let page_count = doc
        .PageCount()
        .map_err(|e| OcrError::PdfOpenFailed(format!("读取页数失败: {e}")))?;
    if page_count == 0 {
        return Err(OcrError::PdfEmptyDocument);
    }

    let page = doc
        .GetPage(0)
        .map_err(|e| OcrError::PdfRenderFailed(format!("GetPage(0) 失败: {e}")))?;

    let size = page
        .Size()
        .map_err(|e| OcrError::PdfRenderFailed(format!("读取页尺寸失败: {e}")))?;
    let (dest_w, dest_h) = compute_render_size(size.Width, size.Height);

    let options = PdfPageRenderOptions::new()
        .map_err(|e| OcrError::PdfRenderFailed(format!("创建渲染选项失败: {e}")))?;
    options
        .SetDestinationWidth(dest_w)
        .map_err(|e| OcrError::PdfRenderFailed(format!("设置 DestinationWidth 失败: {e}")))?;
    options
        .SetDestinationHeight(dest_h)
        .map_err(|e| OcrError::PdfRenderFailed(format!("设置 DestinationHeight 失败: {e}")))?;

    let output = InMemoryRandomAccessStream::new()
        .map_err(|e| OcrError::PdfRenderFailed(format!("创建输出流失败: {e}")))?;
    page.RenderWithOptionsToStreamAsync(&output, &options)
        .map_err(|e| OcrError::PdfRenderFailed(format!("RenderWithOptionsToStreamAsync 失败: {e}")))?
        .get()
        .map_err(|e| OcrError::PdfRenderFailed(format!("PDF 页面渲染失败: {e}")))?;
    output
        .Seek(0)
        .map_err(|e| OcrError::PdfRenderFailed(format!("Seek 输出流失败: {e}")))?;

    let decoder = BitmapDecoder::CreateAsync(&output)
        .map_err(|e| OcrError::PdfRenderFailed(format!("BitmapDecoder::CreateAsync 失败: {e}")))?
        .get()
        .map_err(|e| OcrError::PdfRenderFailed(format!("打开渲染图失败: {e}")))?;

    let width = decoder
        .PixelWidth()
        .map_err(|e| OcrError::PdfRenderFailed(format!("读取像素宽失败: {e}")))?;
    let height = decoder
        .PixelHeight()
        .map_err(|e| OcrError::PdfRenderFailed(format!("读取像素高失败: {e}")))?;
    if width == 0 || height == 0 {
        return Err(OcrError::PdfRenderFailed("渲染结果尺寸为 0".into()));
    }

    let transform = BitmapTransform::new()
        .map_err(|e| OcrError::PdfRenderFailed(format!("创建 BitmapTransform 失败: {e}")))?;
    let provider = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .map_err(|e| OcrError::PdfRenderFailed(format!("GetPixelDataTransformedAsync 失败: {e}")))?
        .get()
        .map_err(|e| OcrError::PdfRenderFailed(format!("读取像素数据失败: {e}")))?;

    let pixel_array = provider
        .DetachPixelData()
        .map_err(|e| OcrError::PdfRenderFailed(format!("DetachPixelData 失败: {e}")))?;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or_else(|| OcrError::PdfRenderFailed("像素尺寸溢出".into()))?;
    if pixel_array.len() != expected {
        return Err(OcrError::PdfRenderFailed(format!(
            "像素字节长度不匹配：期望 {expected}，实际 {}",
            pixel_array.len()
        )));
    }
    let bgra_bytes = pixel_array.as_slice().to_vec();

    // page 实现 IClosable；best-effort 关闭
    let _ = page.Close();

    log::info!(
        "PDF 首页渲染: pages={} size={}x{}",
        page_count,
        width,
        height
    );

    Ok(PdfFirstPage {
        image: CapturedImage {
            bytes: bgra_bytes,
            width,
            height,
            format: CapturedImageFormat::Bgra8,
        },
        page_count,
    })
}

/// 以页逻辑尺寸 ×2（约 192 DPI）为基准，最长边夹到 `VISION_MAX_LONG_EDGE`。
fn compute_render_size(logical_w: f32, logical_h: f32) -> (u32, u32) {
    let mut w = logical_w.max(1.0) * 2.0;
    let mut h = logical_h.max(1.0) * 2.0;
    let long = w.max(h);
    let max = VISION_MAX_LONG_EDGE as f32;
    if long > max {
        let scale = max / long;
        w *= scale;
        h *= scale;
    }
    let dest_w = w.round().clamp(1.0, max) as u32;
    let dest_h = h.round().clamp(1.0, max) as u32;
    (dest_w, dest_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ocr::OcrError;

    fn minimal_one_page_pdf() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/minimal-one-page.pdf"
        ))
        .expect("fixture minimal-one-page.pdf")
    }

    #[tokio::test]
    async fn render_first_page_nonzero_bitmap() {
        let page = render_pdf_first_page(&minimal_one_page_pdf())
            .await
            .expect("render");
        assert!(page.page_count >= 1);
        assert!(page.image.width >= 1 && page.image.height >= 1);
        assert!(!page.image.bytes.is_empty());
        assert_eq!(
            page.image.bytes.len() as u32,
            page.image.width * page.image.height * 4
        );
        assert_eq!(page.image.format, CapturedImageFormat::Bgra8);
        assert!(page.image.width <= VISION_MAX_LONG_EDGE);
        assert!(page.image.height <= VISION_MAX_LONG_EDGE);
    }

    #[tokio::test]
    async fn render_garbage_is_open_failed() {
        let err = render_pdf_first_page(b"not a pdf").await.unwrap_err();
        assert!(matches!(err, OcrError::PdfOpenFailed(_)));
    }

    #[tokio::test]
    async fn render_empty_is_open_failed() {
        let err = render_pdf_first_page(b"").await.unwrap_err();
        assert!(matches!(err, OcrError::PdfOpenFailed(_)));
    }

    #[test]
    fn compute_render_size_clamps_long_edge() {
        let (w, h) = compute_render_size(2000.0, 1000.0);
        // 2x → 4000x2000，再按 2048 缩放
        assert!(w <= VISION_MAX_LONG_EDGE);
        assert!(h <= VISION_MAX_LONG_EDGE);
        assert_eq!(w, VISION_MAX_LONG_EDGE);
        assert!(h > 0);
    }

    #[test]
    fn compute_render_size_small_page_doubles() {
        let (w, h) = compute_render_size(300.0, 144.0);
        assert_eq!(w, 600);
        assert_eq!(h, 288);
    }
}
