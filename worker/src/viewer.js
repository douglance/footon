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
    context.clearRect(0, 0, map.clientWidth, height);
    context.drawImage(texture, 0, 0, map.clientWidth, height);
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
    // The rail reads as evenly spaced dots rather than one solid line: a long
    // thread packs more messages than the rail has room for, so each slot keeps
    // one — the user turn if the slot holds one, otherwise the first message in
    // it, which leaves the mix of agent and tool colors intact.
    const radius = Math.max(1.5, Math.min(2.5, width / 5));
    const spacing = radius * 3;
    const slots = new Map();
    for (const { marker, message } of targets) {
      if (!message || message.hidden) continue;
      const center = (messageTop(message) + message.offsetHeight / 2) * scale;
      const slot = Math.round(center / spacing);
      const rank = marker.classList.contains("user") ? 2 : 1;
      const held = slots.get(slot);
      if (!held || rank > held.rank) slots.set(slot, { rank, marker });
    }
    for (const [slot, { marker }] of slots) {
      textureContext.fillStyle = getComputedStyle(marker).backgroundColor;
      textureContext.beginPath();
      textureContext.arc(width / 2, slot * spacing, radius, 0, Math.PI * 2);
      textureContext.fill();
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

  function setScrollTop(top, behavior = "auto") {
    const maximum = Math.max(0, scrollHeight() - viewportHeight());
    scrollTarget.scrollTo({ top: Math.min(maximum, Math.max(0, top)), behavior });
  }

  function updateScrollbarValue() {
    const maximum = Math.max(0, scrollHeight() - viewportHeight());
    canvas.setAttribute("aria-valuemax", String(Math.round(maximum)));
    canvas.setAttribute("aria-valuenow", String(Math.round(scrollTop())));
  }

  function stopDragging(event) {
    if (!dragging) return;
    dragging = false;
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
  }

  canvas.tabIndex = 0;
  canvas.setAttribute("role", "scrollbar");
  canvas.setAttribute("aria-label", "Thread minimap");
  canvas.setAttribute("aria-controls", viewer.id || "thread-messages");
  canvas.setAttribute("aria-orientation", "vertical");
  canvas.setAttribute("aria-valuemin", "0");
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
  canvas.addEventListener("keydown", (event) => {
    const line = Math.max(40, viewportHeight() / 10);
    const page = Math.max(80, viewportHeight() * 0.8);
    const destinations = {
      ArrowUp: scrollTop() - line,
      ArrowDown: scrollTop() + line,
      PageUp: scrollTop() - page,
      PageDown: scrollTop() + page,
      Home: 0,
      End: scrollHeight(),
    };
    if (!(event.key in destinations)) return;
    event.preventDefault();
    setScrollTop(destinations[event.key]);
  });
  rail?.prepend(canvas);
  for (const filter of filters) filter.addEventListener("change", applyFilters);
  scrollTarget.addEventListener("scroll", updateScrollbarValue, { passive: true });
  addEventListener("resize", layout);
  addEventListener("load", layout, { once: true });
  applyFilters();
  updateScrollbarValue();
}
