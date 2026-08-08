const form = document.querySelector("#search-form");
const submit = document.querySelector("#submit");
const status = document.querySelector("#status");
const resultPanel = document.querySelector(".result");
const empty = document.querySelector("#result-empty");
const content = document.querySelector("#result-content");
const note = document.querySelector("#note");
const metrics = document.querySelector("#metrics");
const json = document.querySelector("#json");
const progressPanel = document.querySelector("#progress-panel");
const progressMessage = document.querySelector("#progress-message");
const progressCount = document.querySelector("#progress-count");
const progressBar = document.querySelector("#progress-bar");
const discoveries = document.querySelector("#discoveries");
const formulaList = document.querySelector("#formula-list");

let worker;
let searchStartedAt = 0;

function idleButtonLabel() {
  return document.querySelector("#mode").value === "exact" ? "全探索を開始" : "PSLQ探索を開始";
}

function coefficientText(coefficient, first) {
  const sign = coefficient < 0 ? "−" : first ? "" : "+";
  const magnitude = Math.abs(coefficient);
  return `${sign}${magnitude === 1 ? "" : magnitude}`;
}

function renderFormula(terms) {
  if (!terms?.length) return "整数係数の π/4 公式は得られませんでした。";
  const right = terms.map((term, index) =>
    `${coefficientText(term.coefficient, index === 0)} arctan(1/${term.denominator})`
  ).join(" ");
  return `π / 4 = ${right}`;
}

function appendFormula(terms, number) {
  const item = document.createElement("li");
  item.className = "formula-item";
  const label = document.createElement("span");
  label.className = "formula-number";
  label.textContent = `#${number}`;
  const expression = document.createElement("span");
  expression.textContent = renderFormula(terms);
  item.append(label, expression);
  formulaList.append(item);
  discoveries.hidden = false;
}

function renderFormulas(found) {
  formulaList.replaceChildren();
  found.forEach((entry, index) => appendFormula(entry.formula, index + 1));
  discoveries.hidden = found.length === 0;
}

function finish(result) {
  resultPanel.setAttribute("aria-busy", "false");
  submit.disabled = false;
  submit.textContent = idleButtonLabel();
  status.textContent = result.formulas.length > 0 ? `${result.formulas.length}件発見` : "未検出";
  status.className = `status ${result.exact ? "success" : "neutral"}`;
  progressPanel.hidden = false;
  progressMessage.textContent = result.note;
  const completedWork = result.processed_states ?? result.iterations ?? 0;
  progressCount.textContent = result.processed_states != null
    ? `${completedWork.toLocaleString()}状態`
    : `${completedWork.toLocaleString()}反復`;
  progressBar.max = 1;
  progressBar.value = 1;
  empty.hidden = true;
  content.hidden = false;
  note.textContent = result.note;
  renderFormulas(result.formulas);
  metrics.innerHTML = result.processed_states != null ? `
      <div><dt>発見公式</dt><dd>${result.formulas.length}件</dd></div>
      <div><dt>処理状態</dt><dd>${result.processed_states.toLocaleString()}</dd></div>
      <div><dt>探索完了率</dt><dd>100%</dd></div>
    ` : `
      <div><dt>発見公式</dt><dd>${result.formulas.length}件</dd></div>
      <div><dt>反復回数</dt><dd>${result.iterations.toLocaleString()}</dd></div>
      <div><dt>残差精度</dt><dd>${result.residual_bits} bit</dd></div>
      <div><dt>有効次元</dt><dd>${result.active_denominators.length + 1}</dd></div>
      <div><dt>デフレーション</dt><dd>${result.deflations.length}回</dd></div>
    `;
  json.textContent = JSON.stringify(result, null, 2);
}

function fail(message) {
  resultPanel.setAttribute("aria-busy", "false");
  submit.disabled = false;
  submit.textContent = idleButtonLabel();
  status.textContent = "エラー";
  status.className = "status error";
  empty.hidden = false;
  empty.textContent = message;
  content.hidden = true;
}

function showProgress(progress) {
  progressPanel.hidden = false;
  progressMessage.textContent = progress.message;
  const processed = progress.processed ?? progress.iteration ?? 0;
  const total = progress.total ?? progress.max_iterations ?? 1;
  const elapsed = Math.max(performance.now() - searchStartedAt, 1);
  const eta = processed > 0 && processed < total
    ? elapsed * (total - processed) / processed
    : 0;
  const etaText = eta > 0
    ? `・残り約${eta >= 60000 ? `${Math.ceil(eta / 60000)}分` : `${Math.ceil(eta / 1000)}秒`}`
    : "";
  progressCount.textContent = `${processed.toLocaleString()} / ${total.toLocaleString()}${etaText}`;
  progressBar.max = Math.max(total, 1);
  progressBar.value = processed;
  if (progress.phase === "formula" && progress.formula) {
    appendFormula(progress.formula, progress.formulas_found);
  }
  status.textContent = progress.phase === "formula"
    ? `${progress.formulas_found}件発見・継続中`
    : progress.phase === "deflation"
      ? "基底を整理中"
      : progress.stage
        ? `探索中・段階${progress.stage}`
        : `全探索中・${Math.floor(processed * 100 / Math.max(total, 1))}%`;
}

function ensureWorker() {
  if (worker) worker.terminate();
  worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker.onmessage = ({ data }) => {
    if (data.type === "progress") {
      showProgress(data.progress);
    } else if (data.ok) {
      finish(data.result);
    } else {
      fail(data.error);
    }
  };
  worker.onerror = () => fail("WASMワーカーを読み込めませんでした。ページを再読み込みしてください。");
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  if (!form.reportValidity()) return;
  ensureWorker();
  searchStartedAt = performance.now();
  const data = new FormData(form);
  resultPanel.setAttribute("aria-busy", "true");
  status.textContent = "計算中";
  status.className = "status running";
  submit.disabled = true;
  submit.textContent = "探索しています…";
  progressPanel.hidden = false;
  progressMessage.textContent = "WASMを読み込んでいます。";
  progressCount.textContent = "";
  progressBar.max = 1;
  progressBar.value = 0;
  formulaList.replaceChildren();
  discoveries.hidden = true;
  empty.hidden = false;
  empty.textContent = "多倍長固定小数点で整数関係を探索中です。";
  content.hidden = true;
  worker.postMessage({
    mode: data.get("mode"),
    denominators: data.get("denominators"),
    maxTerms: Number(data.get("maxTerms")),
    precision: Number(data.get("precision")),
    maxCoeff: Number(data.get("maxCoeff")),
    maxIterations: Number(data.get("maxIterations")),
  });
});

const mode = document.querySelector("#mode");
function updateModeFields() {
  const exact = mode.value === "exact";
  document.querySelector("#max-terms").disabled = !exact;
  document.querySelector("#precision").disabled = exact;
  document.querySelector("#max-iterations").disabled = exact;
  submit.textContent = idleButtonLabel();
}
mode.addEventListener("change", updateModeFields);
updateModeFields();
