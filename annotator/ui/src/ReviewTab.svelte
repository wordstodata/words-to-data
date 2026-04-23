<script>
  let {
    annotations = $bindable([]),
    treeDiff,
    amendments,
    changedNodes = [],
    onLoadLegalDiff,
    onExport,
  } = $props();

  // Build a lookup map from path to diff info
  let diffByPath = $derived.by(() => {
    const lookup = new Map();
    for (const node of changedNodes) {
      lookup.set(node.path, node);
    }
    return lookup;
  });

  let statusFilter = $state("all");
  let expandedIndices = $state(new Set());

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

  // Filtered and sorted annotations
  let filteredAnnotations = $derived.by(() => {
    let result = annotations;
    if (statusFilter !== "all") {
      result = result.filter(
        (ann) => (ann.metadata?.status || "Pending") === statusFilter,
      );
    }
    // Sort by first path
    return [...result].sort((a, b) => {
      const pathA = a.paths?.[0] || "";
      const pathB = b.paths?.[0] || "";
      return pathA.localeCompare(pathB);
    });
  });

  function updateAnnotationStatus(originalIndex, newStatus) {
    annotations = annotations.map((ann, i) => {
      if (i === originalIndex) {
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

  function toggleExpanded(index) {
    const next = new Set(expandedIndices);
    if (next.has(index)) {
      next.delete(index);
    } else {
      next.add(index);
    }
    expandedIndices = next;
  }

  function formatConfidence(confidence) {
    if (confidence === undefined || confidence === null) return null;
    return (confidence * 100).toFixed(0) + "%";
  }

  function getConfidenceClass(confidence) {
    if (confidence >= 0.8) return "high-confidence";
    if (confidence >= 0.5) return "medium-confidence";
    return "low-confidence";
  }

  function isAiAnnotator(annotator) {
    if (!annotator) return false;
    return annotator.startsWith("ai:") || annotator.startsWith("model:");
  }

  function getAnnotatorLabel(annotator) {
    if (!annotator) return "";
    if (annotator.startsWith("model:")) {
      return annotator.substring(6); // Remove "model:" prefix
    }
    if (annotator.startsWith("ai:")) {
      return annotator.substring(3); // Remove "ai:" prefix
    }
    if (annotator.startsWith("human:")) {
      return "Human";
    }
    return annotator;
  }

  // Lookup amendment by ID
  function getAmendmentText(amendmentId) {
    if (!amendments) return null;
    const found = amendments.find((a) => a.id === amendmentId);
    return found?.amending_text || null;
  }

  // Keyboard navigation
  function handleKeydown(e, index) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggleExpanded(index);
    }
  }
</script>

<div class="review-tab">
  <div class="review-toolbar">
    <button class="btn-legal-diff" onclick={onLoadLegalDiff}>
      Load Legal Diff
    </button>
    <button
      class="btn-export"
      onclick={onExport}
      disabled={annotations.length === 0}
    >
      Export ({annotations.length})
    </button>

    <div class="toolbar-separator"></div>

    <label class="filter-label">
      Filter:
      <select bind:value={statusFilter}>
        <option value="all">All ({annotations.length})</option>
        <option value="Pending">Pending ({statusCounts.Pending})</option>
        <option value="Verified">Verified ({statusCounts.Verified})</option>
        <option value="Disputed">Disputed ({statusCounts.Disputed})</option>
        <option value="Rejected">Rejected ({statusCounts.Rejected})</option>
      </select>
    </label>

    <div class="status-summary">
      <span class="status-pill pending">Pending: {statusCounts.Pending}</span>
      <span class="status-pill verified"
        >Verified: {statusCounts.Verified}</span
      >
      <span class="status-pill disputed"
        >Disputed: {statusCounts.Disputed}</span
      >
      <span class="status-pill rejected"
        >Rejected: {statusCounts.Rejected}</span
      >
    </div>
  </div>

  <div class="annotation-cards">
    {#if annotations.length === 0}
      <div class="empty-state">
        <p>No annotations loaded.</p>
        <p>Click "Load Legal Diff" to import annotations for review.</p>
      </div>
    {:else if filteredAnnotations.length === 0}
      <div class="empty-state">
        <p>No annotations match the current filter.</p>
      </div>
    {:else}
      {#each filteredAnnotations as ann, i}
        {@const originalIndex = annotations.indexOf(ann)}
        {@const isExpanded = expandedIndices.has(originalIndex)}
        {@const confidence = ann.metadata?.confidence}
        {@const status = ann.metadata?.status || "Pending"}
        {@const amendmentText = getAmendmentText(ann.source_bill.amendment_id)}
        <div
          class="annotation-card"
          class:expanded={isExpanded}
          class:ai-annotation={isAiAnnotator(ann.metadata?.annotator)}
          class:status-verified={status === "Verified"}
          class:status-rejected={status === "Rejected"}
          class:status-disputed={status === "Disputed"}
        >
          <div class="card-header">
            <span class="ann-index">#{originalIndex + 1}</span>
            <span class="op-tag">{ann.operation}</span>

            {#if confidence !== undefined && confidence !== null}
              <span
                class="confidence-badge {getConfidenceClass(confidence)}"
                title="AI Confidence: {formatConfidence(confidence)}"
              >
                {formatConfidence(confidence)}
              </span>
            {/if}

            {#if ann.metadata?.annotator}
              <span
                class="annotator-tag"
                class:ai-annotator={isAiAnnotator(ann.metadata.annotator)}
                title={ann.metadata.annotator}
              >
                {getAnnotatorLabel(ann.metadata.annotator)}
              </span>
            {/if}

            <div class="card-header-spacer"></div>

            <div class="status-buttons">
              <button
                class="status-btn pending"
                class:active={status === "Pending"}
                onclick={() => updateAnnotationStatus(originalIndex, "Pending")}
                title="Pending"
              >?</button>
              <button
                class="status-btn verified"
                class:active={status === "Verified"}
                onclick={() => updateAnnotationStatus(originalIndex, "Verified")}
                title="Verified"
              >✓</button>
              <button
                class="status-btn disputed"
                class:active={status === "Disputed"}
                onclick={() => updateAnnotationStatus(originalIndex, "Disputed")}
                title="Disputed"
              >!</button>
              <button
                class="status-btn rejected"
                class:active={status === "Rejected"}
                onclick={() => updateAnnotationStatus(originalIndex, "Rejected")}
                title="Rejected"
              >✗</button>
            </div>
          </div>

          <div class="card-body">
            <div class="card-section paths-section">
              <span class="section-label">Paths:</span>
              <div class="paths-list">
                {#each ann.paths as path}
                  {@const diff = diffByPath.get(path)}
                  <div class="path-with-diff">
                    <code class="path-item" title={path}>{path}</code>
                    {#if diff}
                      <div class="unified-diff">
                        {#if diff.changes && diff.changes.length > 0}
                          {#each diff.changes as change}
                            <div class="diff-field-header">{change.field_name}</div>
                            <div class="diff-line diff-remove">- {change.old_value}</div>
                            <div class="diff-line diff-add">+ {change.new_value}</div>
                          {/each}
                        {/if}
                        {#if diff.removed && diff.removed.length > 0}
                          {#each diff.removed as item}
                            <div class="diff-element diff-remove">
                              <div class="diff-element-header">
                                <span class="diff-sign">-</span>
                                <span class="diff-element-type">{item.element_type}</span>
                                {#if item.number_display}
                                  <span class="diff-element-num">{item.number_display}</span>
                                {/if}
                              </div>
                              <div class="diff-element-path">{item.path}</div>
                              <div class="diff-element-fields">
                                {#if item.heading}
                                  <div class="diff-field"><span class="diff-field-label">heading:</span> {item.heading}</div>
                                {/if}
                                {#if item.chapeau}
                                  <div class="diff-field"><span class="diff-field-label">chapeau:</span> {item.chapeau}</div>
                                {/if}
                                {#if item.content}
                                  <div class="diff-field"><span class="diff-field-label">content:</span> {item.content}</div>
                                {/if}
                                {#if item.continuation}
                                  <div class="diff-field"><span class="diff-field-label">continuation:</span> {item.continuation}</div>
                                {/if}
                                {#if item.proviso}
                                  <div class="diff-field"><span class="diff-field-label">proviso:</span> {item.proviso}</div>
                                {/if}
                              </div>
                            </div>
                          {/each}
                        {/if}
                        {#if diff.added && diff.added.length > 0}
                          {#each diff.added as item}
                            <div class="diff-element diff-add">
                              <div class="diff-element-header">
                                <span class="diff-sign">+</span>
                                <span class="diff-element-type">{item.element_type}</span>
                                {#if item.number_display}
                                  <span class="diff-element-num">{item.number_display}</span>
                                {/if}
                              </div>
                              <div class="diff-element-path">{item.path}</div>
                              <div class="diff-element-fields">
                                {#if item.heading}
                                  <div class="diff-field"><span class="diff-field-label">heading:</span> {item.heading}</div>
                                {/if}
                                {#if item.chapeau}
                                  <div class="diff-field"><span class="diff-field-label">chapeau:</span> {item.chapeau}</div>
                                {/if}
                                {#if item.content}
                                  <div class="diff-field"><span class="diff-field-label">content:</span> {item.content}</div>
                                {/if}
                                {#if item.continuation}
                                  <div class="diff-field"><span class="diff-field-label">continuation:</span> {item.continuation}</div>
                                {/if}
                                {#if item.proviso}
                                  <div class="diff-field"><span class="diff-field-label">proviso:</span> {item.proviso}</div>
                                {/if}
                              </div>
                            </div>
                          {/each}
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>

            <div class="card-section">
              <span class="section-label">Causative:</span>
              <span class="causative-text"
                >"{ann.source_bill.causative_text}"</span
              >
            </div>

            <div class="card-section">
              <span class="section-label">Amendment:</span>
              <span class="amendment-id">{ann.source_bill.amendment_id}</span>
            </div>

            {#if amendmentText}
              <details class="amendment-text-section">
                <summary>Full Amendment Text</summary>
                <pre class="amendment-text-content">{amendmentText}</pre>
              </details>
            {/if}

            {#if ann.metadata?.reasoning}
              <details
                class="reasoning-section"
                open={isExpanded}
                ontoggle={(e) => {
                  if (e.target.open && !isExpanded) toggleExpanded(originalIndex);
                  if (!e.target.open && isExpanded) toggleExpanded(originalIndex);
                }}
              >
                <summary
                  tabindex="0"
                  onkeydown={(e) => handleKeydown(e, originalIndex)}
                >
                  Reasoning
                </summary>
                <p class="reasoning-text">{ann.metadata.reasoning}</p>
              </details>
            {/if}

            {#if ann.metadata?.notes}
              <div class="card-section notes-section">
                <span class="section-label">Notes:</span>
                <span class="notes-text">{ann.metadata.notes}</span>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>
