<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";

  // ── file inputs ──────────────────────────────────────────────────────────────
  let oldPath = $state("../tests/test_data/usc/2025-07-18/usc26.xml");
  let oldDate = $state("2025-07-18");
  let newPath = $state("../tests/test_data/usc/2025-07-30/usc26.xml");
  let newDate = $state("2025-07-30");
  let billPath = $state("../tests/test_data/bills/hr-119-21.xml");

  // ── loaded data ───────────────────────────────────────────────────────────────
  let treeDiff = $state(null);
  let billData = $state(null);
  let amendments = $state([]); // Array<BillAmendment>
  let changedNodes = $state([]); // flat list of nodes with changes

  // ── ui state ─────────────────────────────────────────────────────────────────
  let loading = $state(false);
  let error = $state("");
  let nodeFilter = $state("");

  // ── annotation form ───────────────────────────────────────────────────────────
  let selectedAmendment = $state(null);
  let selectedNodePaths = $state(new Set());
  let causativeText = $state("");
  let operation = $state("strike_and_insert");
  let annotator = $state("human:user");
  let notes = $state("");

  // ── saved annotations ─────────────────────────────────────────────────────────
  let annotations = $state([]);

  // ── derived ───────────────────────────────────────────────────────────────────
  let filteredNodes = $derived(
    nodeFilter.trim()
      ? changedNodes.filter((n) => n.path.includes(nodeFilter.trim()))
      : changedNodes,
  );

  let canAnnotate = $derived(
    selectedAmendment !== null &&
      causativeText.trim() !== "" &&
      selectedNodePaths.size > 0,
  );

  // ── helpers ───────────────────────────────────────────────────────────────────
  function extractChangedNodes(diff, results = []) {
    if (
      diff.changes.length > 0 ||
      diff.added.length > 0 ||
      diff.removed.length > 0
    ) {
      results.push({
        path: diff.root_path,
        fieldChanges: diff.changes.length,
        added: diff.added.length,
        removed: diff.removed.length,
        // Keep the first field change for a preview snippet
        preview: diff.changes[0]
          ? `${diff.changes[0].field_name}: "${diff.changes[0].old_value.slice(0, 60)}…"`
          : diff.added.length > 0
            ? `+${diff.added.length} added`
            : `-${diff.removed.length} removed`,
      });
    }
    for (const child of diff.child_diffs) {
      extractChangedNodes(child, results);
    }
    return results;
  }

  function shortPath(path) {
    const parts = path.split("/");
    return parts.length > 3 ? "…/" + parts.slice(-3).join("/") : path;
  }

  // ── file picking ──────────────────────────────────────────────────────────────
  async function pickOldUsc() {
    const p = await open({ multiple: false, title: "Select Old USC XML" });
    if (p) oldPath = p;
  }

  async function pickNewUsc() {
    const p = await open({ multiple: false, title: "Select New USC XML" });
    if (p) newPath = p;
  }

  async function pickBill() {
    const p = await open({
      multiple: false,
      title: "Select Bill XML",
      filters: [{ name: "XML", extensions: ["xml"] }],
    });
    if (p) billPath = p;
  }

  // ── load ──────────────────────────────────────────────────────────────────────
  async function loadFiles() {
    if (!oldPath || !oldDate || !newPath || !newDate || !billPath) {
      error = "Fill in all file paths and dates before loading.";
      return;
    }
    loading = true;
    error = "";
    treeDiff = null;
    billData = null;
    amendments = [];
    changedNodes = [];
    annotations = [];
    selectedAmendment = null;
    selectedNodePaths = new Set();
    causativeText = "";
    try {
      const [diffJson, billJson] = await Promise.all([
        invoke("load_usc_pair", { oldPath, oldDate, newPath, newDate }),
        invoke("load_bill", { path: billPath }),
      ]);
      treeDiff = JSON.parse(diffJson);
      billData = JSON.parse(billJson);
      amendments = Object.values(billData.amendments);
      changedNodes = extractChangedNodes(treeDiff);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // ── annotation form ───────────────────────────────────────────────────────────
  function captureSelection() {
    const sel = window.getSelection()?.toString().trim();
    if (sel) causativeText = sel;
  }

  function toggleNode(path) {
    const next = new Set(selectedNodePaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selectedNodePaths = next;
  }

  async function addAnnotation() {
    if (!canAnnotate) return;
    try {
      const annJson = await invoke("create_annotation", {
        operation,
        billId: billData.bill_id,
        amendmentId: selectedAmendment.id,
        causativeText: causativeText.trim(),
        paths: [...selectedNodePaths],
        annotator: annotator.trim(),
        notes: notes.trim() || null,
      });
      annotations = [...annotations, JSON.parse(annJson)];
      // reset form fields (keep amendment selection for convenience)
      causativeText = "";
      selectedNodePaths = new Set();
      notes = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function exportAnnotations() {
    if (!treeDiff || annotations.length === 0) return;
    const outputPath = await save({
      title: "Save Legal Diff JSON",
      defaultPath: "legal_diff.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!outputPath) return;
    try {
      await invoke("export_legal_diff", {
        treeDiffJson: JSON.stringify(treeDiff),
        annotationsJson: JSON.stringify(annotations),
        outputPath,
      });
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="app">
  <!-- ── toolbar ─────────────────────────────────────────────────────────────── -->
  <div class="toolbar">
    <span class="toolbar-title">USC Annotation Tool</span>

    <div class="field-group">
      <button onclick={pickOldUsc}>Browse…</button>
      <input placeholder="Old USC XML path" bind:value={oldPath} />
      <input
        placeholder="Date (YYYY-MM-DD)"
        bind:value={oldDate}
        class="date-input"
      />
    </div>

    <span class="arrow">→</span>

    <div class="field-group">
      <button onclick={pickNewUsc}>Browse…</button>
      <input placeholder="New USC XML path" bind:value={newPath} />
      <input
        placeholder="Date (YYYY-MM-DD)"
        bind:value={newDate}
        class="date-input"
      />
    </div>

    <div class="field-group">
      <button onclick={pickBill}>Browse…</button>
      <input placeholder="Bill XML path" bind:value={billPath} />
    </div>

    <button class="btn-primary" onclick={loadFiles} disabled={loading}>
      {loading ? "Loading…" : "Generate Diff"}
    </button>
  </div>

  {#if error}
    <div class="error-bar">{error}</div>
  {/if}

  <!-- ── three panels ────────────────────────────────────────────────────────── -->
  <div class="panels">
    <!-- Left: amendment list -->
    <div class="panel panel-left">
      <div class="panel-header">
        Bill Amendments
        {#if billData}
          <span class="badge">{amendments.length}</span>
        {/if}
      </div>
      {#if amendments.length === 0}
        <p class="hint">Load files to see amendments.</p>
      {:else}
        <ul class="amendment-list">
          {#each amendments as amendment (amendment.id)}
            <li
              class="amendment-item"
              class:selected={selectedAmendment?.id === amendment.id}
              onclick={() => (selectedAmendment = amendment)}
            >
              <div class="amendment-ops">
                {#each amendment.action_types as op}
                  <span class="op-tag">{op}</span>
                {/each}
              </div>
              <div class="amendment-preview">
                {amendment.amending_text.slice(0, 100)}{amendment.amending_text
                  .length > 100
                  ? "…"
                  : ""}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Center: amendment text with selectable content -->
    <div class="panel panel-center">
      <div class="panel-header">Amendment Text</div>
      {#if selectedAmendment}
        <div class="selection-hint">
          Select causative text below, then click "Use Selection →"
        </div>
        <pre class="amendment-text">{selectedAmendment.amending_text}</pre>
        <button class="btn-capture" onmousedown={captureSelection}>
          Use Selection →
        </button>
      {:else}
        <p class="hint">Select an amendment on the left to view its text.</p>
      {/if}
    </div>

    <!-- Right: changed diff nodes -->
    <div class="panel panel-right">
      <div class="panel-header">
        Changed Nodes
        {#if changedNodes.length > 0}
          <span class="badge">{changedNodes.length}</span>
        {/if}
      </div>
      {#if changedNodes.length > 0}
        <input
          class="filter-input"
          placeholder="Filter by path…"
          bind:value={nodeFilter}
        />
        <ul class="node-list">
          {#each filteredNodes as node (node.path)}
            <li
              class="node-item"
              class:selected={selectedNodePaths.has(node.path)}
              onclick={() => toggleNode(node.path)}
              title={node.path}
            >
              <div class="node-path">{shortPath(node.path)}</div>
              <div class="node-preview">{node.preview}</div>
              <div class="node-badges">
                {#if node.fieldChanges > 0}<span class="badge-change"
                    >~{node.fieldChanges}</span
                  >{/if}
                {#if node.added > 0}<span class="badge-add">+{node.added}</span
                  >{/if}
                {#if node.removed > 0}<span class="badge-remove"
                    >-{node.removed}</span
                  >{/if}
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="hint">Diff nodes with changes will appear here.</p>
      {/if}
    </div>
  </div>

  <!-- ── annotation bar ─────────────────────────────────────────────────────── -->
  <div class="annotation-bar">
    <div class="ann-field">
      <label>Operation</label>
      <select bind:value={operation}>
        <option value="amend">Amend</option>
        <option value="add">Add</option>
        <option value="delete">Delete</option>
        <option value="insert">Insert</option>
        <option value="redesignate">Redesignate</option>
        <option value="repeal">Repeal</option>
        <option value="move">Move</option>
        <option value="strike">Strike</option>
        <option value="strike_and_insert">Strike & Insert</option>
      </select>
    </div>

    <div class="ann-field causative-field">
      <label>Causative Text</label>
      <input
        placeholder="Select text in center panel, then click 'Use Selection →'"
        bind:value={causativeText}
      />
    </div>

    <div class="ann-field">
      <label>Annotator</label>
      <input bind:value={annotator} class="short-input" />
    </div>

    <div class="ann-field">
      <label>Notes</label>
      <input placeholder="Optional" bind:value={notes} class="short-input" />
    </div>

    <div class="ann-field">
      <label>Paths selected</label>
      <span class="badge" title={[...selectedNodePaths].join("\n")}>
        {selectedNodePaths.size}
      </span>
    </div>

    <button class="btn-primary" onclick={addAnnotation} disabled={!canAnnotate}>
      Add Annotation
    </button>

    <button
      class="btn-export"
      onclick={exportAnnotations}
      disabled={annotations.length === 0}
    >
      Export ({annotations.length})
    </button>
  </div>

  <!-- ── annotation log ────────────────────────────────────────────────────── -->
  {#if annotations.length > 0}
    <div class="annotation-log">
      <div class="panel-header">Annotations ({annotations.length})</div>
      <ul class="annotation-list">
        {#each annotations as ann, i}
          <li class="annotation-entry">
            <span class="ann-index">#{i + 1}</span>
            <span class="ann-op op-tag">{ann.operation}</span>
            <span class="ann-paths" title={ann.paths.join("\n")}>
              {ann.paths.map((p) => shortPath(p)).join(", ")}
            </span>
            <span class="ann-causative"
              >"{ann.source_bill.causative_text.slice(0, 80)}{ann.source_bill
                .causative_text.length > 80
                ? "…"
                : ""}"</span
            >
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  :global(*, *::before, *::after) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family:
      system-ui,
      -apple-system,
      sans-serif;
    font-size: 13px;
    background: #f1f5f9;
    color: #1e293b;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  /* ── toolbar ────────────────────────────────────────────────────────────── */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: #1e293b;
    color: #f8fafc;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .toolbar-title {
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
    margin-right: 4px;
  }

  .field-group {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .toolbar input {
    background: #334155;
    border: 1px solid #475569;
    color: #f8fafc;
    border-radius: 4px;
    padding: 3px 7px;
    font-size: 12px;
    min-width: 200px;
  }

  .toolbar input::placeholder {
    color: #94a3b8;
  }

  .date-input {
    min-width: 110px !important;
  }

  .arrow {
    color: #94a3b8;
    font-size: 16px;
  }

  .error-bar {
    background: #fef2f2;
    color: #991b1b;
    padding: 6px 12px;
    font-size: 12px;
    border-bottom: 1px solid #fecaca;
    flex-shrink: 0;
  }

  /* ── panels ─────────────────────────────────────────────────────────────── */
  .panels {
    display: flex;
    flex: 1;
    overflow: hidden;
    gap: 1px;
    background: #e2e8f0;
  }

  .panel {
    display: flex;
    flex-direction: column;
    background: #fff;
    overflow: hidden;
  }

  .panel-left {
    flex: 0 0 22%;
  }
  .panel-center {
    flex: 1;
  }
  .panel-right {
    flex: 0 0 26%;
  }

  .panel-header {
    padding: 8px 12px;
    font-weight: 600;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #475569;
    background: #f8fafc;
    border-bottom: 1px solid #e2e8f0;
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .hint {
    padding: 16px 12px;
    color: #94a3b8;
    font-size: 12px;
    font-style: italic;
  }

  /* ── amendment list ─────────────────────────────────────────────────────── */
  .amendment-list {
    list-style: none;
    overflow-y: auto;
    flex: 1;
  }

  .amendment-item {
    padding: 8px 12px;
    border-bottom: 1px solid #f1f5f9;
    cursor: pointer;
    transition: background 0.1s;
  }

  .amendment-item:hover {
    background: #f8fafc;
  }
  .amendment-item.selected {
    background: #eff6ff;
    border-left: 3px solid #2563eb;
  }

  .amendment-ops {
    display: flex;
    gap: 4px;
    margin-bottom: 4px;
    flex-wrap: wrap;
  }

  .amendment-preview {
    font-size: 11px;
    color: #64748b;
    line-height: 1.4;
  }

  /* ── center panel ───────────────────────────────────────────────────────── */
  .selection-hint {
    padding: 6px 12px;
    background: #fefce8;
    border-bottom: 1px solid #fef08a;
    font-size: 11px;
    color: #713f12;
    flex-shrink: 0;
  }

  .amendment-text {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    font-family: "Georgia", serif;
    font-size: 13px;
    line-height: 1.7;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  .btn-capture {
    flex-shrink: 0;
    margin: 8px 12px;
    padding: 6px 12px;
    background: #fef3c7;
    border: 1px solid #f59e0b;
    color: #78350f;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
  }

  .btn-capture:hover {
    background: #fde68a;
  }

  /* ── diff node list ─────────────────────────────────────────────────────── */
  .filter-input {
    margin: 8px 12px;
    padding: 4px 8px;
    border: 1px solid #e2e8f0;
    border-radius: 4px;
    font-size: 12px;
    width: calc(100% - 24px);
    flex-shrink: 0;
  }

  .node-list {
    list-style: none;
    overflow-y: auto;
    flex: 1;
  }

  .node-item {
    padding: 7px 12px;
    border-bottom: 1px solid #f1f5f9;
    cursor: pointer;
    transition: background 0.1s;
  }

  .node-item:hover {
    background: #f8fafc;
  }
  .node-item.selected {
    background: #eff6ff;
    border-left: 3px solid #2563eb;
  }

  .node-path {
    font-family: monospace;
    font-size: 11px;
    color: #374151;
    margin-bottom: 2px;
  }

  .node-preview {
    font-size: 10px;
    color: #64748b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 3px;
  }

  .node-badges {
    display: flex;
    gap: 4px;
  }

  /* ── badges ─────────────────────────────────────────────────────────────── */
  .badge {
    background: #e2e8f0;
    color: #475569;
    border-radius: 10px;
    padding: 1px 7px;
    font-size: 11px;
    font-weight: 600;
  }

  .op-tag {
    background: #e0e7ff;
    color: #3730a3;
    border-radius: 3px;
    padding: 1px 5px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge-change {
    background: #fef3c7;
    color: #78350f;
    border-radius: 3px;
    padding: 1px 5px;
    font-size: 10px;
    font-weight: 600;
  }
  .badge-add {
    background: #dcfce7;
    color: #14532d;
    border-radius: 3px;
    padding: 1px 5px;
    font-size: 10px;
    font-weight: 600;
  }
  .badge-remove {
    background: #fef2f2;
    color: #7f1d1d;
    border-radius: 3px;
    padding: 1px 5px;
    font-size: 10px;
    font-weight: 600;
  }

  /* ── buttons ────────────────────────────────────────────────────────────── */
  button {
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    background: #f8fafc;
    font-size: 12px;
    color: #1e293b;
    transition: background 0.1s;
  }

  button:hover:not(:disabled) {
    background: #e2e8f0;
  }
  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #2563eb;
    color: white;
    border-color: #1d4ed8;
    font-weight: 500;
  }

  .btn-primary:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .btn-export {
    background: #16a34a;
    color: white;
    border-color: #15803d;
    font-weight: 500;
  }

  .btn-export:hover:not(:disabled) {
    background: #15803d;
  }

  /* ── annotation bar ─────────────────────────────────────────────────────── */
  .annotation-bar {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    padding: 8px 12px;
    background: #f8fafc;
    border-top: 2px solid #e2e8f0;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .ann-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .ann-field label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #64748b;
  }

  .ann-field select,
  .ann-field input {
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 12px;
    background: white;
    color: #1e293b;
  }

  .causative-field {
    flex: 1;
    min-width: 240px;
  }
  .causative-field input {
    width: 100%;
  }
  .short-input {
    width: 140px;
  }

  /* ── annotation log ─────────────────────────────────────────────────────── */
  .annotation-log {
    border-top: 2px solid #e2e8f0;
    background: #fff;
    max-height: 140px;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .annotation-list {
    list-style: none;
  }

  .annotation-entry {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 5px 12px;
    border-bottom: 1px solid #f1f5f9;
    font-size: 12px;
    line-height: 1.4;
  }

  .ann-index {
    color: #94a3b8;
    font-size: 11px;
    min-width: 24px;
  }

  .ann-paths {
    font-family: monospace;
    font-size: 11px;
    color: #374151;
    min-width: 200px;
  }

  .ann-causative {
    color: #64748b;
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
