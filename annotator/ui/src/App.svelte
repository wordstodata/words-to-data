<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import "./App.css";
  import ReviewTab from "./ReviewTab.svelte";

  // ── tab state ────────────────────────────────────────────────────────────────
  let activeTab = $state("annotate");

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

  // ── annotation review state ──────────────────────────────────────────────────
  let statusFilter = $state("all");
  let isLegalDiffLoaded = $state(false); // Track if we loaded a legal_diff vs workspace

  // Annotation counts per amendment and per path
  let annotationsByAmendment = $derived.by(() => {
    const counts = new Map();
    for (const ann of annotations) {
      const id = ann.source_bill.amendment_id;
      counts.set(id, (counts.get(id) || 0) + 1);
    }
    return counts;
  });

  let annotationsByPath = $derived.by(() => {
    const counts = new Map();
    for (const ann of annotations) {
      for (const path of ann.paths) {
        counts.set(path, (counts.get(path) || 0) + 1);
      }
    }
    return counts;
  });

  // Status counts for annotation review
  let statusCounts = $derived.by(() => {
    const counts = { Pending: 0, Verified: 0, Disputed: 0, Rejected: 0 };
    for (const ann of annotations) {
      const status = ann.metadata?.status || "Pending";
      if (counts[status] !== undefined) {
        counts[status]++;
      }
    }
    return counts;
  });

  // Filtered annotations based on status filter
  let filteredAnnotations = $derived.by(() => {
    if (statusFilter === "all") return annotations;
    return annotations.filter(
      (ann) => (ann.metadata?.status || "Pending") === statusFilter,
    );
  });

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

  // ── mention matches ────────────────────────────────────────────────────────
  // Computed by scanning amendment text for section references
  // Map: amendment_id -> Array<{tree_diff_path, matched_text}>
  let mentionMatches = $state(new Map());
  // Set of amendment IDs that have mention matches
  let amendmentsWithMentions = $derived(new Set(mentionMatches.keys()));
  // Lookup: tree_diff_path -> Set<amendment_id> (reverse lookup)
  let mentionsByPath = $derived.by(() => {
    const lookup = new Map();
    for (const [amendmentId, matches] of mentionMatches.entries()) {
      for (const match of matches) {
        if (!lookup.has(match.tree_diff_path)) {
          lookup.set(match.tree_diff_path, new Set());
        }
        lookup.get(match.tree_diff_path).add(amendmentId);
      }
    }
    return lookup;
  });

  // ── derived ───────────────────────────────────────────────────────────────────
  let filteredNodes = $derived.by(() => {
    let nodes = nodeFilter.trim()
      ? changedNodes.filter((n) => n.path.includes(nodeFilter.trim()))
      : changedNodes;

    // Sort by: 1) similarity scores, 2) regex matches, 3) others
    if (selectedAmendment) {
      const scoresForAmendment =
        similarityByAmendment.get(selectedAmendment.id) ?? new Map();
      const mentionsForAmendment =
        mentionMatches.get(selectedAmendment.id) ?? [];
      const mentionedPaths = new Set(
        mentionsForAmendment.map((m) => m.tree_diff_path),
      );

      nodes = [...nodes].sort((a, b) => {
        const scoreA = scoresForAmendment.get(a.path)?.score ?? -1;
        const scoreB = scoresForAmendment.get(b.path)?.score ?? -1;
        const aMention = mentionedPaths.has(a.path);
        const bMention = mentionedPaths.has(b.path);

        // Primary: similarity scores (higher first)
        if (scoreA > 0 || scoreB > 0) {
          if (scoreA !== scoreB) return scoreB - scoreA;
        }
        // Secondary: regex matches
        if (aMention !== bMention) return bMention ? 1 : -1;
        // Tertiary: keep original order
        return 0;
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

    // Sort amendments: mentions first, then similarity scores, then rest
    result = [...result].sort((a, b) => {
      // Mentions get highest priority
      const aHasMentions = amendmentsWithMentions.has(a.id) ? 2 : 0;
      const bHasMentions = amendmentsWithMentions.has(b.id) ? 2 : 0;
      // Scores get secondary priority
      const aHasScores = amendmentsWithScores.has(a.id) ? 1 : 0;
      const bHasScores = amendmentsWithScores.has(b.id) ? 1 : 0;
      return bHasMentions + bHasScores - (aHasMentions + aHasScores);
    });
    return result;
  });

  let canAnnotate = $derived(
    selectedAmendment !== null &&
      causativeText.trim() !== "" &&
      selectedNodePaths.size > 0,
  );

  // ── helpers ───────────────────────────────────────────────────────────────────
  function removeAmendmentFilter() {
    if (amendmentFilters.length > 1) {
      amendmentFilters = amendmentFilters.slice(0, -1);
    }
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
        added: diff.added,
        removed: diff.removed,
        changes: diff.changes,
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

  function getMentionMatch(nodePath) {
    if (!selectedAmendment || !mentionMatches.has(selectedAmendment.id)) {
      return null;
    }
    const matches = mentionMatches.get(selectedAmendment.id);
    return matches.find((m) => m.tree_diff_path === nodePath) ?? null;
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

  async function scanMentions() {
    if (!treeDiff || billsData.length === 0) {
      error = "Load files first before scanning mentions.";
      return;
    }
    try {
      const mentionsJson = await invoke("scan_amendments_for_mentions", {
        treeDiffJson: JSON.stringify(treeDiff),
        billsJson: JSON.stringify(billsData),
      });
      mentionMatches = new Map(Object.entries(JSON.parse(mentionsJson)));
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
    mentionMatches = new Map();
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
        billsJson: JSON.stringify(billsData),
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
      mentionMatches = new Map();

      // Reset selection state
      selectedAmendment = null;
      selectedNodePaths = new Set();
      causativeText = "";
      isLegalDiffLoaded = false;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadLegalDiff() {
    const inputPath = await open({
      title: "Load Legal Diff",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!inputPath) return;
    loading = true;
    error = "";
    try {
      const legalDiffJson = await invoke("load_legal_diff", { path: inputPath });
      const legalDiff = JSON.parse(legalDiffJson);

      // Populate state from LegalDiff
      treeDiff = legalDiff.tree_diff;
      annotations = legalDiff.annotations || [];

      // Convert amendments dict to array for UI
      amendments = Object.values(legalDiff.amendments || {});

      // Create a synthetic billsData structure for compatibility
      // The amendments are stored flat in LegalDiff, so we create a single pseudo-bill
      billsData = [
        {
          bill_id: "legal_diff",
          amendments: legalDiff.amendments || {},
        },
      ];
      billPaths = [];

      // Extract changed nodes from the tree diff
      changedNodes = extractChangedNodes(treeDiff);
      mentionMatches = new Map();

      // Reset selection state
      selectedAmendment = null;
      selectedNodePaths = new Set();
      causativeText = "";
      statusFilter = "all";
      isLegalDiffLoaded = true;

      // Clear USC paths since we loaded from a LegalDiff
      oldPath = "";
      oldDate = "";
      newPath = "";
      newDate = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function updateAnnotationStatus(index, newStatus) {
    // Create a new annotations array with the updated status
    annotations = annotations.map((ann, i) => {
      if (i === index) {
        return {
          ...ann,
          metadata: {
            ...ann.metadata,
            status: newStatus,
          },
        };
      }
      return ann;
    });
  }
</script>

<div class="app">
  <!-- ── tab bar ─────────────────────────────────────────────────────────────── -->
  <div class="tab-bar">
    <button
      class:active={activeTab === "annotate"}
      onclick={() => (activeTab = "annotate")}
    >
      Annotate
    </button>
    <button
      class:active={activeTab === "review"}
      onclick={() => (activeTab = "review")}
    >
      Review
    </button>
  </div>

  {#if activeTab === "annotate"}
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
      <button class="btn-legal-diff" onclick={loadLegalDiff} disabled={loading}>
        Load Legal Diff
      </button>
      {#if isLegalDiffLoaded}
        <span class="badge badge-legal-diff">Review Mode</span>
      {/if}
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
      <button
        class="btn-mentions"
        onclick={scanMentions}
        disabled={!treeDiff || billsData.length === 0}
      >
        Scan Mentions
      </button>
      {#if mentionMatches.size > 0}
        <span class="badge badge-mentions">{mentionMatches.size} matches</span>
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
                {#if annotationsByAmendment.get(amendment.id)}
                  <span class="badge-annotated"
                    >{annotationsByAmendment.get(amendment.id)}✓</span
                  >
                {/if}
                {#if mentionMatches.has(amendment.id)}
                  <span
                    class="mention-tag"
                    title={mentionMatches
                      .get(amendment.id)
                      .map((m) => `${m.matched_text} → ${m.tree_diff_path}`)
                      .join("\n")}
                    >🔗{mentionMatches.get(amendment.id).length}</span
                  >
                {/if}
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
            {@const mention = getMentionMatch(node.path)}
            <li
              class="node-item"
              class:selected={selectedNodePaths.has(node.path)}
              class:has-similarity={similarity !== null}
              class:has-mention={mention !== null}
              onclick={() => toggleNode(node.path)}
              onkeydown={(e) => e.key === "Enter" && toggleNode(node.path)}
              role="option"
              aria-selected={selectedNodePaths.has(node.path)}
              tabindex="0"
              title={node.path}
            >
              {#if similarity || mention}
                <div class="node-match-tags">
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
                  {#if mention}
                    <span
                      class="mention-badge"
                      title="Mentioned as: {mention.matched_text}"
                    >
                      🔗 {mention.matched_text}
                    </span>
                  {/if}
                </div>
              {/if}
              <div class="node-header">
                <div class="node-path">{shortPath(node.path)}</div>
                <div class="node-header-badges">
                  {#if annotationsByPath.get(node.path)}
                    <span class="badge-annotated"
                      >{annotationsByPath.get(node.path)}✓</span
                    >
                  {/if}
                </div>
              </div>
              {#if node.changes.length > 0}
                <div class="node-changes">
                  {#each node.changes as change}
                    <div class="change-entry">
                      <span class="field-name">{change.field_name}:</span>
                      <span class="old-val">{change.old_value}</span>
                      <span class="change-arrow">→</span>
                      <span class="new-val">{change.new_value}</span>
                    </div>
                  {/each}
                </div>
              {/if}
              <div class="node-badges">
                {#if node.fieldChanges > 0}<span class="badge-change"
                    >~{node.fieldChanges}</span
                  >{/if}
              </div>
              {#if node.added.length > 0}
                <div class="added-removed-list">
                  {#each node.added as elem}
                    <span class="badge-add">+{shortPath(elem.path)}</span>
                  {/each}
                </div>
              {/if}
              {#if node.removed.length > 0}
                <div class="added-removed-list">
                  {#each node.removed as elem}
                    <span class="badge-remove">-{shortPath(elem.path)}</span>
                  {/each}
                </div>
              {/if}
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
      <div class="panel-header">
        Annotations ({annotations.length})
        <div class="status-filter">
          <select bind:value={statusFilter}>
            <option value="all">All</option>
            <option value="Pending">Pending</option>
            <option value="Verified">Verified</option>
            <option value="Disputed">Disputed</option>
            <option value="Rejected">Rejected</option>
          </select>
          <span class="status-counts">
            Pending: {statusCounts.Pending} |
            Verified: {statusCounts.Verified} |
            Disputed: {statusCounts.Disputed} |
            Rejected: {statusCounts.Rejected}
          </span>
        </div>
      </div>
      <ul class="annotation-list">
        {#each filteredAnnotations as ann, i}
          {@const originalIndex = annotations.indexOf(ann)}
          <li
            class="annotation-entry"
            class:ai-annotation={ann.metadata?.annotator?.startsWith("ai:")}
            class:status-verified={ann.metadata?.status === "Verified"}
            class:status-rejected={ann.metadata?.status === "Rejected"}
            class:status-disputed={ann.metadata?.status === "Disputed"}
          >
            <div class="ann-main-row">
              <span class="ann-index">#{originalIndex + 1}</span>
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

              {#if ann.metadata?.confidence !== undefined && ann.metadata.confidence !== null}
                <span
                  class="confidence-badge"
                  class:high-confidence={ann.metadata.confidence >= 0.8}
                  class:medium-confidence={ann.metadata.confidence >= 0.5 &&
                    ann.metadata.confidence < 0.8}
                  class:low-confidence={ann.metadata.confidence < 0.5}
                >
                  {(ann.metadata.confidence * 100).toFixed(0)}%
                </span>
              {/if}

              <select
                class="status-select"
                value={ann.metadata?.status || "Pending"}
                onchange={(e) =>
                  updateAnnotationStatus(originalIndex, e.target.value)}
              >
                <option value="Pending">Pending</option>
                <option value="Verified">Verified</option>
                <option value="Disputed">Disputed</option>
                <option value="Rejected">Rejected</option>
              </select>
            </div>

            {#if ann.metadata?.reasoning}
              <details class="ann-reasoning">
                <summary>Reasoning</summary>
                <p>{ann.metadata.reasoning}</p>
              </details>
            {/if}

            {#if ann.metadata?.notes}
              <div class="ann-notes">
                <span class="notes-label">Notes:</span> {ann.metadata.notes}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  {/if}
  {/if}

  {#if activeTab === "review"}
    <ReviewTab
      bind:annotations
      {treeDiff}
      {amendments}
      {changedNodes}
      onLoadLegalDiff={loadLegalDiff}
      onExport={exportAnnotations}
    />
  {/if}
</div>
