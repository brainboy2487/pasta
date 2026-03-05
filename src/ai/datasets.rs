// src/ai/datasets.rs
//! Dataset loading and preprocessing utilities for PASTA AI utilities
//!
//! This module provides a small, pragmatic dataset API used by the lightweight
//! autograd/tensor utilities and example models. It supports:
//! - Loading tabular datasets from CSV and JSON Lines (ndjson).
//! - Basic preprocessing: numeric column selection, normalization (min-max, z-score),
//!   shuffling, train/test split, and batching.
//! - A simple `Dataset` iterator that yields (features, labels) batches as `Vec<f64>`
//!   and shape metadata.
//!
//! Design notes:
//! - The implementation favors clarity and portability over heavy dependencies.
//!   It uses `serde` + `csv` for CSV parsing and `serde_json` for ndjson.
//! - For randomness (shuffling) it uses the runtime RNG (`crate::runtime::rng::Rng`)
//!   so device-specific hardware RNGs are used when available.
//! - The API is intentionally small so it can be extended to support images,
//!   TFRecord, or other formats later.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::collections::HashMap;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::runtime::rng::Rng;

/// How to normalize numeric columns.
#[derive(Debug, Clone, Copy)]
pub enum Normalization {
    None,
    MinMax,   // scale to [0,1]
    ZScore,   // (x - mean) / std
}

/// A simple in-memory tabular dataset.
///
/// `features` is a flat vector of length `n_rows * n_features` in row-major order.
/// `labels` is optional and is a flat vector of length `n_rows * n_label_dims`.
#[derive(Debug, Clone)]
pub struct Dataset {
    pub n_rows: usize,
    pub n_features: usize,
    pub n_label_dims: usize,
    pub features: Vec<f64>,
    pub labels: Option<Vec<f64>>,
    /// Column names for features (optional)
    pub feature_names: Vec<String>,
    /// Label names (optional)
    pub label_names: Vec<String>,
}

impl Dataset {
    /// Create an empty dataset.
    pub fn new() -> Self {
        Self {
            n_rows: 0,
            n_features: 0,
            n_label_dims: 0,
            features: Vec::new(),
            labels: None,
            feature_names: Vec::new(),
            label_names: Vec::new(),
        }
    }

    /// Load a CSV file from `path`.
    ///
    /// `label_columns` may be:
    /// - `Some(vec!["y"])` to treat column "y" as label(s)
    /// - `None` to treat all numeric columns as features and no labels
    ///
    /// `has_header` indicates whether the CSV has a header row.
    pub fn from_csv<P: AsRef<Path>>(path: P, label_columns: Option<Vec<&str>>, has_header: bool) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(has_header)
            .from_path(path.as_ref())?;

        let headers: Vec<String> = if has_header {
            rdr.headers()?.iter().map(|s| s.to_string()).collect()
        } else {
            // generate generic names
            let first = rdr.headers()?;
            (0..first.len()).map(|i| format!("col{}", i)).collect()
        };

        // Determine label indices
        let mut label_idx: Vec<usize> = Vec::new();
        if let Some(cols) = &label_columns {
            for c in cols {
                if let Some(pos) = headers.iter().position(|h| h == c) {
                    label_idx.push(pos);
                } else {
                    return Err(anyhow!("label column '{}' not found in CSV headers", c));
                }
            }
        }

        let mut feature_names: Vec<String> = Vec::new();
        let mut label_names: Vec<String> = Vec::new();
        for (i, h) in headers.iter().enumerate() {
            if label_idx.contains(&i) {
                label_names.push(h.clone());
            } else {
                feature_names.push(h.clone());
            }
        }

        let mut features: Vec<Vec<f64>> = Vec::new();
        let mut labels: Vec<Vec<f64>> = if label_idx.is_empty() { Vec::new() } else { Vec::new() };

        for result in rdr.records() {
            let rec = result?;
            let mut row_feats: Vec<f64> = Vec::new();
            let mut row_labels: Vec<f64> = Vec::new();
            for (i, field) in rec.iter().enumerate() {
                // Try to parse numeric; if not numeric, skip or treat as NaN
                let parsed = field.trim().parse::<f64>();
                let val = match parsed {
                    Ok(v) => v,
                    Err(_) => {
                        // Non-numeric: attempt to map booleans, else NaN
                        if field.eq_ignore_ascii_case("true") {
                            1.0
                        } else if field.eq_ignore_ascii_case("false") {
                            0.0
                        } else {
                            f64::NAN
                        }
                    }
                };
                if label_idx.contains(&i) {
                    row_labels.push(val);
                } else {
                    row_feats.push(val);
                }
            }
            features.push(row_feats);
            if !label_idx.is_empty() {
                labels.push(row_labels);
            }
        }

        // Validate consistent row lengths
        if features.is_empty() {
            return Err(anyhow!("CSV contains no data rows"));
        }
        let n_rows = features.len();
        let n_features = features[0].len();
        for r in &features {
            if r.len() != n_features {
                return Err(anyhow!("inconsistent feature column count in CSV"));
            }
        }
        let n_label_dims = if !labels.is_empty() { labels[0].len() } else { 0 };
        if !labels.is_empty() {
            for r in &labels {
                if r.len() != n_label_dims {
                    return Err(anyhow!("inconsistent label column count in CSV"));
                }
            }
        }

        // Flatten into row-major vectors
        let mut flat_feats = Vec::with_capacity(n_rows * n_features);
        for r in features {
            flat_feats.extend_from_slice(&r);
        }
        let flat_labels = if !labels.is_empty() {
            let mut fl = Vec::with_capacity(n_rows * n_label_dims);
            for r in labels {
                fl.extend_from_slice(&r);
            }
            Some(fl)
        } else {
            None
        };

        Ok(Self {
            n_rows,
            n_features,
            n_label_dims,
            features: flat_feats,
            labels: flat_labels,
            feature_names,
            label_names,
        })
    }

    /// Load a JSON Lines (ndjson) file where each line is a JSON object with numeric fields.
    ///
    /// `feature_columns` and `label_columns` specify which keys to use.
    pub fn from_ndjson<P: AsRef<Path>>(path: P, feature_columns: Vec<&str>, label_columns: Option<Vec<&str>>) -> Result<Self> {
        let f = File::open(path)?;
        let reader = BufReader::new(f);
        let mut features: Vec<Vec<f64>> = Vec::new();
        let mut labels: Vec<Vec<f64>> = Vec::new();

        for line in reader.lines() {
            let l = line?;
            if l.trim().is_empty() { continue; }
            let v: serde_json::Value = serde_json::from_str(&l)?;
            let mut row_feats = Vec::with_capacity(feature_columns.len());
            for &k in &feature_columns {
                let val = v.get(k).and_then(|vv| vv.as_f64()).unwrap_or(f64::NAN);
                row_feats.push(val);
            }
            features.push(row_feats);

            if let Some(lcols) = &label_columns {
                let mut row_labels = Vec::with_capacity(lcols.len());
                for &k in lcols {
                    let val = v.get(k).and_then(|vv| vv.as_f64()).unwrap_or(f64::NAN);
                    row_labels.push(val);
                }
                labels.push(row_labels);
            }
        }

        if features.is_empty() {
            return Err(anyhow!("ndjson contains no data rows"));
        }

        let n_rows = features.len();
        let n_features = features[0].len();
        for r in &features {
            if r.len() != n_features {
                return Err(anyhow!("inconsistent feature column count in ndjson"));
            }
        }

        let n_label_dims = if !labels.is_empty() { labels[0].len() } else { 0 };
        if !labels.is_empty() {
            for r in &labels {
                if r.len() != n_label_dims {
                    return Err(anyhow!("inconsistent label column count in ndjson"));
                }
            }
        }

        let mut flat_feats = Vec::with_capacity(n_rows * n_features);
        for r in features {
            flat_feats.extend_from_slice(&r);
        }
        let flat_labels = if !labels.is_empty() {
            let mut fl = Vec::with_capacity(n_rows * n_label_dims);
            for r in labels {
                fl.extend_from_slice(&r);
            }
            Some(fl)
        } else {
            None
        };

        Ok(Self {
            n_rows,
            n_features,
            n_label_dims,
            features: flat_feats,
            labels: flat_labels,
            feature_names: feature_columns.iter().map(|s| s.to_string()).collect(),
            label_names: label_columns.unwrap_or_default().iter().map(|s| s.to_string()).collect(),
        })
    }

    /// Normalize features in-place according to `norm`.
    ///
    /// Returns a map of (col_index -> (mean, std, min, max)) for possible inverse transforms.
    pub fn normalize(&mut self, norm: Normalization) -> HashMap<usize, (f64, f64, f64, f64)> {
        let mut stats: HashMap<usize, (f64, f64, f64, f64)> = HashMap::new();
        if norm == Normalization::None {
            return stats;
        }
        for col in 0..self.n_features {
            // collect column values
            let mut col_vals = Vec::with_capacity(self.n_rows);
            for r in 0..self.n_rows {
                let v = self.features[r * self.n_features + col];
                col_vals.push(v);
            }
            // compute stats ignoring NaN
            let mut sum = 0.0;
            let mut cnt = 0usize;
            let mut min = std::f64::INFINITY;
            let mut max = std::f64::NEG_INFINITY;
            for &v in &col_vals {
                if v.is_nan() { continue; }
                sum += v;
                cnt += 1;
                if v < min { min = v; }
                if v > max { max = v; }
            }
            let mean = if cnt > 0 { sum / (cnt as f64) } else { 0.0 };
            let mut var_sum = 0.0;
            for &v in &col_vals {
                if v.is_nan() { continue; }
                let d = v - mean;
                var_sum += d * d;
            }
            let std = if cnt > 1 { (var_sum / ((cnt - 1) as f64)).sqrt() } else { 0.0 };

            // apply normalization
            for r in 0..self.n_rows {
                let idx = r * self.n_features + col;
                let v = self.features[idx];
                if v.is_nan() { continue; }
                let nv = match norm {
                    Normalization::MinMax => {
                        if (max - min).abs() < std::f64::EPSILON { 0.0 } else { (v - min) / (max - min) }
                    }
                    Normalization::ZScore => {
                        if std.abs() < std::f64::EPSILON { 0.0 } else { (v - mean) / std }
                    }
                    Normalization::None => v,
                };
                self.features[idx] = nv;
            }

            stats.insert(col, (mean, std, min, max));
        }
        stats
    }

    /// Shuffle dataset rows in-place using the provided RNG.
    pub fn shuffle_inplace(&mut self, rng: &mut Rng) {
        if self.n_rows <= 1 { return; }
        // Fisher-Yates shuffle
        for i in (1..self.n_rows).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            if i == j { continue; }
            // swap feature rows
            for c in 0..self.n_features {
                let a = i * self.n_features + c;
                let b = j * self.n_features + c;
                self.features.swap(a, b);
            }
            // swap labels if present
            if let Some(ref mut labs) = self.labels {
                for c in 0..self.n_label_dims {
                    let a = i * self.n_label_dims + c;
                    let b = j * self.n_label_dims + c;
                    labs.swap(a, b);
                }
            }
        }
    }

    /// Split dataset into (train, test) by fraction `train_frac` in [0,1].
    ///
    /// If `shuffle` is true, dataset is shuffled first using RNG.
    pub fn train_test_split(&self, train_frac: f64, shuffle: bool, rng: &mut Rng) -> Result<(Dataset, Dataset)> {
        if !(0.0..=1.0).contains(&train_frac) {
            return Err(anyhow!("train_frac must be in [0,1]"));
        }
        let mut ds = self.clone();
        if shuffle {
            ds.shuffle_inplace(rng);
        }
        let n_train = ((ds.n_rows as f64) * train_frac).round() as usize;
        let n_test = ds.n_rows - n_train;

        let mut train = Dataset::new();
        train.n_rows = n_train;
        train.n_features = ds.n_features;
        train.n_label_dims = ds.n_label_dims;
        train.feature_names = ds.feature_names.clone();
        train.label_names = ds.label_names.clone();
        train.features = Vec::with_capacity(n_train * ds.n_features);
        if ds.labels.is_some() {
            train.labels = Some(Vec
