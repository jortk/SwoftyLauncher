(() => {
  if (window.__swoftyAds) return;
  window.__swoftyAds = true;
  const isTop = window.top === window;
  const activated = () => !navigator.userActivation || navigator.userActivation.isActive;
  const send = (raw) => {
    let url;
    try {
      url = new URL(String(raw), location.href).href;
    } catch (_) {
      return;
    }
    if (!/^https?:/.test(url)) return;
    if (isTop) {
      if (activated() && window.ipc) window.ipc.postMessage(url);
    } else {
      window.top.postMessage({ swoftyAdClick: url }, "*");
    }
  };
  if (isTop) {
    window.addEventListener("message", (event) => {
      const url = event.data && event.data.swoftyAdClick;
      if (typeof url === "string") send(url);
    });
  }
  document.addEventListener(
    "click",
    (event) => {
      if (!event.isTrusted) return;
      let node = event.target;
      if (node && node.nodeType !== 1) node = node.parentElement;
      const link = node && node.closest && node.closest("a[href]");
      if (!link) return;
      event.preventDefault();
      event.stopPropagation();
      send(link.href);
    },
    true,
  );
  window.open = function (url) {
    if (url && activated()) send(url);
    return null;
  };
})();
