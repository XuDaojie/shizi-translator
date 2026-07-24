namespace Shizi.Popup.State;

/// <summary>
/// sessionId 形如 <c>batchId:serviceInstanceId</c>。
/// 对齐 Vue <c>batchIdFromSession</c>：非字符串/无冒号返回 null；取第一个 <c>:</c> 前缀。
/// </summary>
public static class SessionIds
{
    public static string? BatchIdFromSession(string? sessionId)
    {
        if (sessionId is null)
            return null;

        var idx = sessionId.IndexOf(':');
        if (idx == -1)
            return null;

        return sessionId[..idx];
    }
}
