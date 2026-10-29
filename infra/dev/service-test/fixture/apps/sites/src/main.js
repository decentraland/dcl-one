export const message = 'one';
document.querySelector('#app').textContent = message;
if (import.meta.hot) import.meta.hot.accept();
