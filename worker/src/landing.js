(() => {
  async function copyText(text) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    const field = document.createElement("textarea");
    field.value = text;
    field.setAttribute("readonly", "");
    field.style.position = "fixed";
    field.style.opacity = "0";
    document.body.append(field);
    field.select();
    const copied = document.execCommand("copy");
    field.remove();
    if (!copied) throw new Error("copy failed");
  }

  document.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-copy-target]");
    if (!button) return;

    const target = document.getElementById(button.dataset.copyTarget);
    if (!target) return;

    const label = button.textContent;
    const status = button.parentElement.querySelector("[data-copy-status]");
    try {
      await copyText(target.textContent.trim());
      button.textContent = "COPIED";
      if (status) status.textContent = "Agent prompt copied.";
    } catch {
      button.textContent = "COPY FAILED";
      if (status) status.textContent = "Agent prompt could not be copied.";
    }

    window.setTimeout(() => {
      button.textContent = label;
      if (status) status.textContent = "";
    }, 1800);
  });
})();
