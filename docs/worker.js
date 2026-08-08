import init, { search_exact, search_pslq } from "./pkg/pi_arctan.js";

let initialized;

self.onmessage = async ({ data }) => {
  try {
    initialized ??= init();
    await initialized;
    const report = (progress) => self.postMessage({
        type: "progress",
        progress: JSON.parse(progress),
      });
    const raw = data.mode === "exact"
      ? search_exact(data.denominators, data.maxTerms, data.maxCoeff, report)
      : search_pslq(
          data.denominators,
          data.precision,
          data.maxCoeff,
          data.maxIterations,
          report,
        );
    self.postMessage({ type: "result", ok: true, result: JSON.parse(raw) });
  } catch (error) {
    self.postMessage({ type: "result", ok: false, error: String(error) });
  }
};
