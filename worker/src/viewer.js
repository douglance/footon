for (const viewer of document.querySelectorAll(".viewer")) {
  initializeViewer(viewer);
}

function initializeViewer(viewer) {
  const rail = viewer.querySelector(".minimap");
  const map = rail?.querySelector("ol");
  const markers = [...viewer.querySelectorAll(".map-marker")];
  const targets = markers.map((marker) => ({
    marker,
    message: viewer.querySelector(
      `#${marker.getAttribute("href")?.slice(1) || marker.dataset.messageId}`,
    ),
  }));
  const filters = [...viewer.querySelectorAll("[data-filter-role]")];
  const scrollsItself = viewer.dataset.threadScroll === "true";
  const scrollTarget = scrollsItself ? viewer : window;
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d", { alpha: true });
  const texture = document.createElement("canvas");
  const textureContext = texture.getContext("2d", { alpha: true });
  let scale = 1;
  let frame = 0;
  let dragging = false;

  function viewportHeight() {
    return scrollsItself ? viewer.clientHeight : innerHeight;
  }

  function scrollHeight() {
    return scrollsItself ? viewer.scrollHeight : document.documentElement.scrollHeight;
  }

  function scrollTop() {
    return scrollsItself ? viewer.scrollTop : scrollY;
  }

  function messageTop(message) {
    const top = message.getBoundingClientRect().top;
    if (!scrollsItself) return top + scrollY;
    return top - viewer.getBoundingClientRect().top + viewer.scrollTop;
  }

  function render() {
    const height = map.clientHeight;
    const viewport = Math.min(height, viewportHeight() * scale);
    const top = Math.min(
      height - viewport,
      Math.max(0, scrollTop() * scale),
    );
    context.clearRect(0, 0, map.clientWidth, height);
    context.drawImage(texture, 0, 0, map.clientWidth, height);
    context.fillStyle = "rgba(255,255,255,.2)";
    context.fillRect(0, top, map.clientWidth, viewport);
  }

  function schedule() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      render();
    });
  }

  function layout() {
    if (!rail || !map || !context || !textureContext) return;
    scale = map.clientHeight / scrollHeight();
    const ratio = devicePixelRatio || 1;
    const width = map.clientWidth;
    const height = map.clientHeight;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    texture.width = canvas.width;
    texture.height = canvas.height;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    textureContext.setTransform(ratio, 0, 0, ratio, 0, 0);
    textureContext.clearRect(0, 0, width, height);
    for (const { marker, message } of targets) {
      if (!message || message.hidden) continue;
      textureContext.fillStyle = getComputedStyle(marker).backgroundColor;
      textureContext.fillRect(
        1,
        messageTop(message) * scale,
        Math.max(1, width - 2),
        Math.max(1, message.offsetHeight * scale),
      );
    }
    render();
    rail.classList.add("enhanced");
  }

  function filterRole(message) {
    if (message.classList.contains("user")) return "user";
    if (message.classList.contains("assistant")) return "assistant";
    return "tool";
  }

  function applyFilters() {
    const enabled = new Set(
      filters.filter((filter) => filter.checked).map((filter) => filter.dataset.filterRole),
    );
    for (const { marker, message } of targets) {
      if (!message) continue;
      const hidden = !enabled.has(filterRole(message));
      message.hidden = hidden;
      marker.closest("li")?.toggleAttribute("hidden", hidden);
    }
    for (const run of viewer.querySelectorAll(".activity-run")) {
      run.hidden = ![...run.querySelectorAll(".message")].some(
        (message) => !message.hidden,
      );
    }
    for (const block of viewer.querySelectorAll(".call-block")) {
      block.hidden = ![...block.querySelectorAll(".message")].some(
        (message) => !message.hidden,
      );
    }
    requestAnimationFrame(layout);
  }

  function seek(event, behavior) {
    const bounds = canvas.getBoundingClientRect();
    const y = Math.min(bounds.height, Math.max(0, event.clientY - bounds.top));
    const top = y / scale - viewportHeight() / 2;
    scrollTarget.scrollTo({ top, behavior });
  }

  function stopDragging(event) {
    if (!dragging) return;
    dragging = false;
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
  }

  canvas.tabIndex = 0;
  canvas.setAttribute("role", "navigation");
  canvas.setAttribute("aria-label", "Thread minimap");
  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    dragging = true;
    canvas.setPointerCapture(event.pointerId);
    seek(event, "auto");
  });
  canvas.addEventListener("pointermove", (event) => {
    if (dragging) seek(event, "auto");
  });
  canvas.addEventListener("pointerup", stopDragging);
  canvas.addEventListener("pointercancel", stopDragging);
  rail?.prepend(canvas);
  for (const filter of filters) filter.addEventListener("change", applyFilters);
  scrollTarget.addEventListener("scroll", schedule, { passive: true });
  addEventListener("resize", layout);
  addEventListener("load", layout, { once: true });
  applyFilters();
}
