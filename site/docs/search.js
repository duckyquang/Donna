// Docs search: filters the generated window.DOCS_INDEX as you type.
(() => {
  const input = document.querySelector("[data-docs-search]");
  if (!input || !window.DOCS_INDEX) return;

  const box = document.createElement("div");
  box.className = "search-results";
  box.hidden = true;
  input.parentElement.appendChild(box);

  const render = (items, query) => {
    box.innerHTML = "";
    if (!query) {
      box.hidden = true;
      return;
    }
    if (items.length === 0) {
      box.innerHTML = '<div class="search-empty">No results. Try another word?</div>';
      box.hidden = false;
      return;
    }
    for (const item of items.slice(0, 8)) {
      const a = document.createElement("a");
      a.href = `${item.page}#${item.id}`;
      a.textContent = item.q;
      const where = document.createElement("span");
      where.textContent = item.pageTitle;
      a.appendChild(where);
      box.appendChild(a);
    }
    box.hidden = false;
  };

  const score = (item, terms) => {
    const q = item.q.toLowerCase();
    const text = item.text.toLowerCase();
    let s = 0;
    for (const t of terms) {
      if (q.includes(t)) s += 3;
      else if (text.includes(t)) s += 1;
      else return 0; // every term must match somewhere
    }
    return s;
  };

  input.addEventListener("input", () => {
    const query = input.value.trim().toLowerCase();
    const terms = query.split(/\s+/).filter(Boolean);
    if (terms.length === 0) return render([], "");
    const hits = window.DOCS_INDEX.map((item) => ({ item, s: score(item, terms) }))
      .filter((h) => h.s > 0)
      .sort((a, b) => b.s - a.s)
      .map((h) => h.item);
    render(hits, query);
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      input.value = "";
      render([], "");
      input.blur();
    }
    if (e.key === "Enter") {
      const first = box.querySelector("a");
      if (first) window.location.href = first.href;
    }
  });

  document.addEventListener("click", (e) => {
    if (!input.parentElement.contains(e.target)) box.hidden = true;
  });
})();
