// Bewegungskrieg Wiki — search, sortable tables, filters, theme. No dependencies; works from file://.
(function () {
  "use strict";

  // ---------- theme
  var root = document.documentElement;
  var toggle = document.getElementById("theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var current = root.getAttribute("data-theme") ||
        (window.matchMedia && matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try { localStorage.setItem("bk-wiki-theme", next); } catch (e) {}
    });
  }

  // ---------- show code: reveal the variable behind each number
  var codeBtn = document.getElementById("code-toggle");
  function syncCode() {
    if (codeBtn) codeBtn.classList.toggle("on", root.classList.contains("show-code"));
  }
  if (codeBtn) {
    codeBtn.addEventListener("click", function () {
      root.classList.toggle("show-code");
      try { localStorage.setItem("bk-wiki-code", root.classList.contains("show-code") ? "1" : "0"); } catch (e) {}
      syncCode();
    });
    syncCode();
  }

  // ---------- search
  var input = document.getElementById("search");
  var box = document.getElementById("search-results");
  var index = window.WIKI_INDEX || [];
  var sel = -1;

  function score(item, q) {
    var t = item.t.toLowerCase();
    if (t === q) return 100;
    if (t.indexOf(q) === 0) return 60 - Math.min(t.length, 40) / 10;
    var words = t.split(/[\s_\-]+/);
    for (var i = 0; i < words.length; i++) if (words[i].indexOf(q) === 0) return 40;
    if (t.indexOf(q) >= 0) return 25;
    if ((item.x || "").toLowerCase().indexOf(q) >= 0) return 8;
    return 0;
  }
  var kindRank = { Unit: 6, Building: 5, Faction: 7, Research: 4, Ability: 4, Page: 5, Weapon: 3, Constant: 0 };

  function esc(s) {
    return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; });
  }

  function run() {
    var q = input.value.trim().toLowerCase();
    if (!q) { box.hidden = true; return; }
    var hits = [];
    for (var i = 0; i < index.length; i++) {
      var sc = score(index[i], q);
      if (sc > 0) hits.push([sc + (kindRank[index[i].k] || 0), index[i]]);
    }
    hits.sort(function (a, b) { return b[0] - a[0]; });
    hits = hits.slice(0, 14);
    sel = hits.length ? 0 : -1;
    box.innerHTML = hits.length ? hits.map(function (h, i) {
      var it = h[1];
      var extra = it.k === "Constant" ? '<span class="x">' + esc(it.x) + "</span>" : "";
      return '<a href="' + esc(it.u) + '"' + (i === 0 ? ' class="sel"' : "") + "><span>" + esc(it.t) + "</span>" + extra + '<span class="k">' + esc(it.k) + "</span></a>";
    }).join("") : '<div class="empty">No matches</div>';
    box.hidden = false;
  }

  function move(d) {
    var links = box.querySelectorAll("a");
    if (!links.length) return;
    if (sel >= 0) links[sel].classList.remove("sel");
    sel = (sel + d + links.length) % links.length;
    links[sel].classList.add("sel");
    links[sel].scrollIntoView({ block: "nearest" });
  }

  if (input && box) {
    input.addEventListener("input", run);
    input.addEventListener("focus", run);
    input.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown") { e.preventDefault(); move(1); }
      else if (e.key === "ArrowUp") { e.preventDefault(); move(-1); }
      else if (e.key === "Enter") {
        var links = box.querySelectorAll("a");
        if (sel >= 0 && links[sel]) location.href = links[sel].getAttribute("href");
      } else if (e.key === "Escape") { box.hidden = true; input.blur(); }
    });
    document.addEventListener("click", function (e) {
      if (!box.contains(e.target) && e.target !== input) box.hidden = true;
    });
    document.addEventListener("keydown", function (e) {
      var tag = (document.activeElement && document.activeElement.tagName) || "";
      if (e.key === "/" && tag !== "INPUT" && tag !== "TEXTAREA") { e.preventDefault(); input.focus(); input.select(); }
    });
  }

  // ---------- sortable tables
  function cellValue(td) {
    if (!td) return "";
    var v = td.getAttribute("data-v");
    if (v === null) v = td.textContent.trim();
    v = String(v).trim();
    if (/^[\s—\-]*$/.test(v)) return Infinity;
    if (/^-?\d/.test(v)) return parseFloat(v);
    return v.toLowerCase();
  }
  Array.prototype.forEach.call(document.querySelectorAll("table.sortable"), function (table) {
    var heads = table.querySelectorAll("thead th");
    Array.prototype.forEach.call(heads, function (th, col) {
      th.addEventListener("click", function () {
        var asc = !th.classList.contains("asc");
        Array.prototype.forEach.call(heads, function (h) { h.classList.remove("asc", "desc"); });
        th.classList.add(asc ? "asc" : "desc");
        var body = table.tBodies[0];
        var rows = Array.prototype.slice.call(body.rows);
        rows.sort(function (a, b) {
          var x = cellValue(a.cells[col]), y = cellValue(b.cells[col]);
          var r;
          if (typeof x === "number" && typeof y === "number") r = x - y;
          else r = String(x).localeCompare(String(y));
          return asc ? r : -r;
        });
        rows.forEach(function (r) { body.appendChild(r); });
      });
    });
  });

  // ---------- comparison table filters
  Array.prototype.forEach.call(document.querySelectorAll(".filters[data-filter-table]"), function (bar) {
    var table = document.getElementById(bar.getAttribute("data-filter-table"));
    var faction = "";
    var text = bar.querySelector("[data-filter-text]");
    function apply() {
      var q = text ? text.value.trim().toLowerCase() : "";
      Array.prototype.forEach.call(table.tBodies[0].rows, function (r) {
        var okF = !faction || (" " + r.getAttribute("data-faction") + " ").indexOf(" " + faction + " ") >= 0;
        var okT = !q || r.textContent.toLowerCase().indexOf(q) >= 0;
        r.classList.toggle("hidden", !(okF && okT));
      });
    }
    Array.prototype.forEach.call(bar.querySelectorAll("button"), function (b) {
      b.addEventListener("click", function () {
        Array.prototype.forEach.call(bar.querySelectorAll("button"), function (x) { x.classList.remove("on"); });
        b.classList.add("on");
        faction = b.getAttribute("data-faction");
        apply();
      });
    });
    if (text) text.addEventListener("input", apply);
  });

  // ---------- changelog filters (type, faction, text)
  var cbar = document.querySelector("[data-filter-changes]");
  if (cbar) {
    var kind = "", fac = "", ctext = cbar.querySelector("[data-filter-text]");
    function applyChanges() {
      var q = ctext ? ctext.value.trim().toLowerCase() : "";
      Array.prototype.forEach.call(document.querySelectorAll(".version"), function (v) {
        var any = false, groups = v.querySelectorAll(".cg");
        Array.prototype.forEach.call(groups, function (g) {
          var ok = (!kind || g.getAttribute("data-kind") === kind) &&
            (!fac || (" " + g.getAttribute("data-faction") + " ").indexOf(" " + fac + " ") >= 0) &&
            (!q || g.textContent.toLowerCase().indexOf(q) >= 0);
          g.classList.toggle("hidden", !ok);
          if (ok) any = true;
        });
        v.classList.toggle("hidden", groups.length > 0 && !any && !!(kind || fac || q));
      });
    }
    Array.prototype.forEach.call(cbar.querySelectorAll("button"), function (b) {
      b.addEventListener("click", function () {
        if (b.hasAttribute("data-kind")) {
          Array.prototype.forEach.call(cbar.querySelectorAll("button[data-kind]"), function (x) { x.classList.remove("on"); });
          b.classList.add("on"); kind = b.getAttribute("data-kind");
        } else {
          var was = b.classList.contains("on");
          Array.prototype.forEach.call(cbar.querySelectorAll("button[data-faction]"), function (x) { x.classList.remove("on"); });
          if (!was) b.classList.add("on");
          fac = was ? "" : b.getAttribute("data-faction");
        }
        applyChanges();
      });
    });
    if (ctext) ctext.addEventListener("input", applyChanges);
  }

  // ---------- constants filter
  var cf = document.querySelector("[data-filter-consts]");
  if (cf) {
    cf.addEventListener("input", function () {
      var q = cf.value.trim().toLowerCase();
      Array.prototype.forEach.call(document.querySelectorAll(".const-group"), function (g) {
        var any = false;
        Array.prototype.forEach.call(g.querySelectorAll("tbody tr"), function (r) {
          var ok = !q || r.textContent.toLowerCase().indexOf(q) >= 0;
          r.classList.toggle("hidden", !ok);
          if (ok) any = true;
        });
        g.classList.toggle("hidden", !any);
      });
    });
  }

  // ---------- open collapsed sections that hold the linked row
  function openTarget() {
    if (!location.hash) return;
    var el = document.getElementById(decodeURIComponent(location.hash.slice(1)));
    for (var p = el; p; p = p.parentElement) if (p.tagName === "DETAILS") p.open = true;
    if (el) el.scrollIntoView({ block: "center" });
  }
  window.addEventListener("hashchange", openTarget);
  openTarget();
})();
