<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import "./App.css";

  // ── file inputs ──────────────────────────────────────────────────────────────
  let oldPath = $state("../tests/test_data/usc/2025-07-18/usc26.xml");
  let oldDate = $state("2025-07-18");
  let newPath = $state("../tests/test_data/usc/2025-07-30/usc26.xml");
  let newDate = $state("2025-07-30");
  let billPaths = $state(["../tests/test_data/bills/hr-119-21.xml"]);

  // ── loaded data ───────────────────────────────────────────────────────────────
  let treeDiff = $state(null);
  let billsData = $state([]); // Array<AmendmentData>
  let amendments = $state([]); // Array<BillAmendment> (flattened from all bills)
  let changedNodes = $state([]); // flat list of nodes with changes

  // ── ui state ─────────────────────────────────────────────────────────────────
  let loading = $state(false);
  let error = $state("");
  let nodeFilter = $state("");
  let amendmentFilters = $state([""]);

  // ── annotation form ───────────────────────────────────────────────────────────
  let selectedAmendment = $state(null);
  let selectedNodePaths = $state(new Set());
  let causativeText = $state("");
  let operation = $state("strike_and_insert");
  let annotator = $state("human:user");
  let notes = $state("");

  // ── saved annotations ─────────────────────────────────────────────────────────
  let annotations = $state([]);

  // ── similarity scores ───────────────────────────────────────────────────────
  // Loaded from JSON: Array<{tree_diff_path, amendment_id, score, precision, recall, ...}>
  let similarityScores = $state([]);
  // Lookup: amendment_id -> tree_diff_path -> score data
  let similarityByAmendment = $derived.by(() => {
    const lookup = new Map();
    for (const s of similarityScores) {
      if (!lookup.has(s.amendment_id)) {
        lookup.set(s.amendment_id, new Map());
      }
      lookup.get(s.amendment_id).set(s.tree_diff_path, s);
    }
    return lookup;
  });
  // Set of amendment IDs that have similarity scores
  let amendmentsWithScores = $derived(
    new Set(similarityScores.map((s) => s.amendment_id)),
  );

  // ── derived ───────────────────────────────────────────────────────────────────
  let filteredNodes = $derived.by(() => {
    let nodes = nodeFilter.trim()
      ? changedNodes.filter((n) => n.path.includes(nodeFilter.trim()))
      : changedNodes;

    // Sort by similarity score if an amendment is selected and we have scores
    if (selectedAmendment && similarityByAmendment.has(selectedAmendment.id)) {
      const scoresForAmendment = similarityByAmendment.get(
        selectedAmendment.id,
      );
      nodes = [...nodes].sort((a, b) => {
        const scoreA = scoresForAmendment.get(a.path)?.score ?? -1;
        const scoreB = scoresForAmendment.get(b.path)?.score ?? -1;
        return scoreB - scoreA; // Descending
      });
    }
    return nodes;
  });

  let filteredAmendments = $derived.by(() => {
    let result = amendmentFilters.join("")
      ? amendments.filter((n) =>
          amendmentFilters.every((f) => {
            return n.amending_text.includes(f);
          }),
        )
      : amendments;

    // Sort amendments with similarity scores to the top
    if (amendmentsWithScores.size > 0) {
      result = [...result].sort((a, b) => {
        const aHasScores = amendmentsWithScores.has(a.id) ? 1 : 0;
        const bHasScores = amendmentsWithScores.has(b.id) ? 1 : 0;
        return bHasScores - aHasScores;
      });
    }
    return result;
  });

  let canAnnotate = $derived(
    selectedAmendment !== null &&
      causativeText.trim() !== "" &&
      selectedNodePaths.size > 0,
  );

  // ── helpers ───────────────────────────────────────────────────────────────────
  function removeAmendmentFilter(index) {
    amendmentFilters = amendmentFilters.filter((_, i) => i !== index);
  }
  function addAmendmentFilter() {
    // We use the spread operator to create a new array reference.
    // Svelte needs a new assignment to trigger a reactivity update.
    amendmentFilters = [...amendmentFilters, ""];
  }
  function addBillPath() {
    billPaths = [...billPaths, ""];
  }
  function removeBillPath(index) {
    if (billPaths.length > 1) {
      billPaths = billPaths.filter((_, i) => i !== index);
    }
  }
  function getBillIdForAmendment(amendment) {
    // Find which bill this amendment belongs to by checking billsData
    for (const bill of billsData) {
      if (bill.amendments[amendment.id]) {
        return bill.bill_id;
      }
    }
    return "unknown";
  }
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
        changes: diff.changes,
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
    return parts.length > 6 ? "…/" + parts.slice(-6).join("/") : path;
  }

  function getSimilarityScore(nodePath) {
    if (
      !selectedAmendment ||
      !similarityByAmendment.has(selectedAmendment.id)
    ) {
      return null;
    }
    return (
      similarityByAmendment.get(selectedAmendment.id).get(nodePath) ?? null
    );
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

  async function pickBill(index) {
    const p = await open({
      multiple: false,
      title: "Select Bill XML",
      filters: [{ name: "XML", extensions: ["xml"] }],
    });
    if (p) {
      billPaths = billPaths.map((path, i) => (i === index ? p : path));
    }
  }

  async function pickBillsMultiple() {
    const paths = await open({
      multiple: true,
      title: "Select Bill XML Files",
      filters: [{ name: "XML", extensions: ["xml"] }],
    });
    if (paths && paths.length > 0) {
      billPaths = [...billPaths.filter((p) => p.trim() !== ""), ...paths];
    }
  }

  async function loadSimilarityScores() {
    const inputPath = await open({
      title: "Load Similarity Scores JSON",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!inputPath) return;
    try {
      const content = await invoke("read_json_file", { path: inputPath });
      similarityScores = JSON.parse(content);
    } catch (e) {
      error = String(e);
    }
  }

  // ── load ──────────────────────────────────────────────────────────────────────
  async function loadFiles() {
    const validBillPaths = billPaths.filter((p) => p.trim() !== "");
    if (
      !oldPath ||
      !oldDate ||
      !newPath ||
      !newDate ||
      validBillPaths.length === 0
    ) {
      error = "Fill in all file paths and dates before loading.";
      return;
    }
    loading = true;
    error = "";
    treeDiff = null;
    billsData = [];
    amendments = [];
    changedNodes = [];
    annotations = [];
    selectedAmendment = null;
    selectedNodePaths = new Set();
    causativeText = "";
    try {
      const [diffJson, billsJson] = await Promise.all([
        invoke("load_usc_pair", { oldPath, oldDate, newPath, newDate }),
        invoke("load_bills", { paths: validBillPaths }),
      ]);
      treeDiff = JSON.parse(diffJson);
      billsData = JSON.parse(billsJson);
      // Flatten amendments from all bills into a single array
      amendments = billsData.flatMap((bill) => Object.values(bill.amendments));
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
      const billId = getBillIdForAmendment(selectedAmendment);
      const annJson = await invoke("create_annotation", {
        operation,
        billId,
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

  // ── workspace save/load ─────────────────────────────────────────────────────
  async function saveWorkspace() {
    if (!treeDiff) {
      error = "Load files first before saving workspace.";
      return;
    }
    const outputPath = await save({
      title: "Save Workspace",
      defaultPath: "workspace.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!outputPath) return;
    try {
      await invoke("save_workspace", {
        treeDiffJson: JSON.stringify(treeDiff),
        uscOldPath: oldPath,
        uscOldDate: oldDate,
        uscNewPath: newPath,
        uscNewDate: newDate,
        billsJson: JSON.stringify(billsData),
        billPaths: billPaths.filter((p) => p.trim() !== ""),
        annotationsJson: JSON.stringify(annotations),
        outputPath,
      });
    } catch (e) {
      error = String(e);
    }
  }

  async function loadWorkspace() {
    const inputPath = await open({
      title: "Load Workspace",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!inputPath) return;
    loading = true;
    error = "";
    try {
      const workspaceJson = await invoke("load_workspace", { path: inputPath });
      const workspace = JSON.parse(workspaceJson);

      // Restore all state from workspace
      treeDiff = workspace.tree_diff;
      oldPath = workspace.usc_old_path;
      oldDate = workspace.usc_old_date;
      newPath = workspace.usc_new_path;
      newDate = workspace.usc_new_date;
      billsData = workspace.bills;
      billPaths = workspace.bill_paths;
      annotations = workspace.annotations;

      // Reconstruct derived state
      amendments = billsData.flatMap((bill) => Object.values(bill.amendments));
      changedNodes = extractChangedNodes(treeDiff);

      // Reset selection state
      selectedAmendment = null;
      selectedNodePaths = new Set();
      causativeText = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
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

    <div class="bill-inputs">
      <span class="bill-label">Bills:</span>
      {#each billPaths as _, i}
        <div class="bill-input-row">
          <button onclick={() => pickBill(i)}>Browse…</button>
          <input placeholder="Bill XML path" bind:value={billPaths[i]} />
          {#if billPaths.length > 1}
            <button
              class="btn-small btn-danger"
              onclick={() => removeBillPath(i)}>-</button
            >
          {/if}
        </div>
      {/each}
      <div class="bill-buttons">
        <button class="btn-small" onclick={addBillPath}>+ Add Bill</button>
        <button class="btn-small" onclick={pickBillsMultiple}
          >+ Add Multiple…</button
        >
      </div>
    </div>

    <button class="btn-primary" onclick={loadFiles} disabled={loading}>
      {loading ? "Loading…" : "Generate Diff"}
    </button>

    <div class="workspace-buttons">
      <button
        class="btn-workspace"
        onclick={saveWorkspace}
        disabled={!treeDiff}
      >
        Save Workspace
      </button>
      <button class="btn-workspace" onclick={loadWorkspace} disabled={loading}>
        Load Workspace
      </button>
      <button
        class="btn-workspace"
        onclick={loadSimilarityScores}
        disabled={loading}
      >
        Load Scores
      </button>
      {#if similarityScores.length > 0}
        <span class="badge badge-scores">{similarityScores.length} scores</span>
      {/if}
    </div>
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
        {#if billsData.length > 0}
          <span class="badge">{amendments.length}</span>
          <span class="badge badge-bills"
            >{billsData.length} bill{billsData.length > 1 ? "s" : ""}</span
          >
        {/if}
      </div>
      {#if amendments.length === 0}
        <p class="hint">Load files to see amendments.</p>
      {:else}
        <div class="input-group">
          {#each amendmentFilters as _, i}
            <div>
              <input
                bind:value={amendmentFilters[i]}
                placeholder="Enter text..."
              />
            </div>
          {/each}
          <button class="btn-primary" onmousedown={addAmendmentFilter}>+</button
          >
          <button class="btn-danger" onmousedown={removeAmendmentFilter}
            >-</button
          >
        </div>
        <ul class="amendment-list" role="listbox">
          {#each filteredAmendments as amendment (amendment.id)}
            <li
              class="amendment-item"
              class:selected={selectedAmendment?.id === amendment.id}
              class:has-scores={amendmentsWithScores.has(amendment.id)}
              onclick={() => (selectedAmendment = amendment)}
              onkeydown={(e) =>
                e.key === "Enter" && (selectedAmendment = amendment)}
              role="option"
              aria-selected={selectedAmendment?.id === amendment.id}
              tabindex="0"
            >
              <div class="amendment-ops">
                {#if amendmentsWithScores.has(amendment.id)}
                  <span class="score-tag">📊</span>
                {/if}
                {#if billsData.length > 1}
                  <span class="bill-tag"
                    >{getBillIdForAmendment(amendment)}</span
                  >
                {/if}
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
        <ul class="node-list" role="listbox" aria-multiselectable="true">
          {#each filteredNodes as node (node.path)}
            {@const similarity = getSimilarityScore(node.path)}
            <li
              class="node-item"
              class:selected={selectedNodePaths.has(node.path)}
              class:has-similarity={similarity !== null}
              onclick={() => toggleNode(node.path)}
              onkeydown={(e) => e.key === "Enter" && toggleNode(node.path)}
              role="option"
              aria-selected={selectedNodePaths.has(node.path)}
              tabindex="0"
              title={node.path}
            >
              <div class="node-header">
                <div class="node-path">{shortPath(node.path)}</div>
                {#if similarity}
                  <span
                    class="similarity-score"
                    title="Score: {similarity.score.toFixed(
                      3,
                    )}, Precision: {similarity.precision.toFixed(
                      3,
                    )}, Recall: {similarity.recall.toFixed(3)}"
                  >
                    {(similarity.score * 100).toFixed(0)}%
                  </span>
                {/if}
              </div>
              <div class="node-changes">
                {#each node.changes as change}
                  <div class="change-entry">
                    <span class="field-name">{change.field_name}:</span>
                    <span class="old-val">{change.old_value}</span>
                    <span class="change-arrow">→</span>
                    <span class="new-val">{change.new_value}</span>
                  </div>
                {/each}
                {#if node.changes.length === 0}
                  <div class="node-preview">{node.preview}</div>
                {/if}
              </div>
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
      <label for="operation">Operation</label>
      <select id="operation" bind:value={operation}>
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
      <label for="causative-text">Causative Text</label>
      <input
        id="causative-text"
        placeholder="Select text in center panel, then click 'Use Selection →'"
        bind:value={causativeText}
      />
    </div>

    <div class="ann-field">
      <label for="annotator">Annotator</label>
      <input id="annotator" bind:value={annotator} class="short-input" />
    </div>

    <div class="ann-field">
      <label for="notes">Notes</label>
      <input
        id="notes"
        placeholder="Optional"
        bind:value={notes}
        class="short-input"
      />
    </div>

    <div class="ann-field">
      <span class="ann-label">Paths selected</span>
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
