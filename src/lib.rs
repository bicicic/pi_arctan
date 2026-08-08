use wasm_bindgen::prelude::*;

pub mod core;
pub mod exact;
pub mod machin;
pub mod pslq;

use crate::core::parse_denominators;

fn emit_json<T: serde::Serialize>(callback: &js_sys::Function, event: &T) {
    if let Ok(json) = serde_json::to_string(event) {
        let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&json));
    }
}

/// Runs deflating PSLQ and returns every exact formula found as JSON.
#[wasm_bindgen]
pub fn search_pslq(
    denominator_specification: &str,
    precision_digits: u32,
    max_coefficient: u32,
    max_iterations: u32,
    progress_callback: &js_sys::Function,
) -> Result<String, JsValue> {
    let denominators = parse_denominators(denominator_specification)
        .map_err(|message| JsValue::from_str(&message))?;
    let result = machin::search(
        denominators,
        precision_digits as usize,
        i64::from(max_coefficient),
        max_iterations as usize,
        |event| emit_json(progress_callback, &event),
    )
    .map_err(|message| JsValue::from_str(&message))?;
    serde_json::to_string(&result).map_err(|error| JsValue::from_str(&error.to_string()))
}

/// Exhaustively searches the bounded denominator/coefficient/support space.
#[wasm_bindgen]
pub fn search_exact(
    denominator_specification: &str,
    max_terms: u32,
    coefficient_limit: u32,
    progress_callback: &js_sys::Function,
) -> Result<String, JsValue> {
    let denominators = parse_denominators(denominator_specification)
        .map_err(|message| JsValue::from_str(&message))?;
    let result = exact::search(
        denominators,
        max_terms as usize,
        coefficient_limit,
        |event| emit_json(progress_callback, &event),
    )
    .map_err(|message| JsValue::from_str(&message))?;
    serde_json::to_string(&result).map_err(|error| JsValue::from_str(&error.to_string()))
}
