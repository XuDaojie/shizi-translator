const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const translateBtn = document.getElementById('translateBtn');
const clearBtn = document.getElementById('clearBtn');

translateBtn.addEventListener('click', () => {
  const text = inputText.value.trim();
  if (!text) {
    outputText.textContent = '请输入要翻译的文本';
    outputText.style.color = '#999';
    return;
  }
  outputText.textContent = `[翻译占位] ${text}`;
  outputText.style.color = '#333';
});

clearBtn.addEventListener('click', () => {
  inputText.value = '';
  outputText.textContent = '翻译结果将显示在这里';
  outputText.style.color = '#999';
});

inputText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    translateBtn.click();
  }
});
