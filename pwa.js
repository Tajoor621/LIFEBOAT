(() => {
  const status = document.getElementById("network");
  const install = document.getElementById("install");
  let installPrompt = null;
  const updateNetwork = () => {
    const online = navigator.onLine;
    status.textContent = online ? "Online" : "Offline · shell available";
    status.dataset.state = online ? "online" : "offline";
  };
  window.addEventListener("online", updateNetwork);
  window.addEventListener("offline", updateNetwork);
  updateNetwork();
  window.addEventListener("beforeinstallprompt", event => {
    event.preventDefault();
    installPrompt = event;
    install.hidden = false;
  });
  install.addEventListener("click", async () => {
    if (!installPrompt) return;
    installPrompt.prompt();
    await installPrompt.userChoice;
    installPrompt = null;
    install.hidden = true;
  });
  window.addEventListener("appinstalled", () => {
    install.hidden = true;
    installPrompt = null;
  });
})();
