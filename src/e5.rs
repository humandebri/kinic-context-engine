// Where: src/e5.rs
// What: Local multilingual-e5-large embedding runtime backed by ONNX Runtime.
// Why: Keep query, document, and section embeddings in Rust so the CLI owns local inference.
use anyhow::{Context, Result, anyhow, bail};
use ndarray::{Array2, Array3, Axis};
use ort::{session::Session, value::TensorRef};
use std::{
    env,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tokenizers::{EncodeInput, PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use crate::embed_protocol::EMBEDDING_DIM;

const DEFAULT_MODEL_DIR: &str = ".local/models/multilingual-e5-large";
const MAX_LENGTH: usize = 512;

pub fn default_model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_MODEL_DIR)
}

pub fn resolved_model_dir() -> PathBuf {
    env::var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(default_model_dir)
}

pub fn embed_text(text: &str) -> Result<Vec<f32>> {
    embed_text_with_model_dir(resolved_model_dir(), text)
}

pub fn embed_text_with_model_dir(model_dir: PathBuf, text: &str) -> Result<Vec<f32>> {
    let cache = runtime_cache().lock().expect("runtime cache lock");
    let mut cache = cache;
    if cache
        .as_ref()
        .is_none_or(|runtime| runtime.model_dir != model_dir)
    {
        *cache = Some(E5Runtime::from_dir(&model_dir)?);
    }
    cache.as_mut().expect("runtime should exist").embed(text)
}

fn runtime_cache() -> &'static Mutex<Option<E5Runtime>> {
    static CACHE: OnceLock<Mutex<Option<E5Runtime>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

struct E5Runtime {
    model_dir: PathBuf,
    tokenizer: Tokenizer,
    session: Session,
    input_names: Vec<String>,
}

impl E5Runtime {
    fn from_dir(model_dir: &Path) -> Result<Self> {
        ensure_model_files(model_dir)?;
        let tokenizer = load_tokenizer(model_dir)?;
        let session = Session::builder()
            .context("failed to create ONNX session builder")?
            .commit_from_file(resolve_onnx_path(model_dir)?)
            .context("failed to load ONNX model")?;
        let input_names = session
            .inputs()
            .iter()
            .map(|item| item.name().to_string())
            .collect::<Vec<_>>();
        Ok(Self {
            model_dir: model_dir.to_path_buf(),
            tokenizer,
            session,
            input_names,
        })
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let encoded = self
            .tokenizer
            .encode(EncodeInput::Single(text.into()), true)
            .map_err(|error| anyhow!("failed to tokenize text: {error}"))?;
        let token_count = encoded.len();
        if token_count == 0 {
            bail!("tokenizer produced an empty sequence");
        }

        let input_ids = Array2::from_shape_vec(
            (1, token_count),
            encoded.get_ids().iter().map(|value| i64::from(*value)).collect(),
        )
        .context("failed to build input_ids tensor")?;
        let attention_mask = Array2::from_shape_vec(
            (1, token_count),
            encoded
                .get_attention_mask()
                .iter()
                .map(|value| i64::from(*value))
                .collect(),
        )
        .context("failed to build attention_mask tensor")?;
        let token_type_ids = Array2::<i64>::zeros((1, token_count));

        let outputs = if self.input_names.iter().any(|name| name == "token_type_ids") {
            self.session.run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(input_ids.view())?,
                "attention_mask" => TensorRef::from_array_view(attention_mask.view())?,
                "token_type_ids" => TensorRef::from_array_view(token_type_ids.view())?,
            ])?
        } else {
            self.session.run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(input_ids.view())?,
                "attention_mask" => TensorRef::from_array_view(attention_mask.view())?,
            ])?
        };
        let hidden = outputs[0]
            .try_extract_array::<f32>()
            .context("failed to extract last_hidden_state")?
            .into_dimensionality::<ndarray::Ix3>()
            .context("unexpected ONNX output rank")?;
        mean_pool_and_normalize(hidden.to_owned(), attention_mask)
    }
}

fn ensure_model_files(model_dir: &Path) -> Result<()> {
    if !model_dir.exists() {
        bail!(
            "embedding model directory not found: {}",
            model_dir.display()
        );
    }
    let tokenizer = model_dir.join("tokenizer.json");
    let config = model_dir.join("config.json");
    if !tokenizer.is_file() {
        bail!("missing tokenizer.json in {}", model_dir.display());
    }
    if !config.is_file() {
        bail!("missing config.json in {}", model_dir.display());
    }
    let _ = resolve_onnx_path(model_dir)?;
    Ok(())
}

fn load_tokenizer(model_dir: &Path) -> Result<Tokenizer> {
    let mut tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
        .map_err(|error| anyhow!("failed to load tokenizer.json: {error}"))?;
    tokenizer.with_padding(Some(PaddingParams {
        strategy: PaddingStrategy::BatchLongest,
        ..Default::default()
    }));
    tokenizer
        .with_truncation(Some(TruncationParams {
            max_length: MAX_LENGTH,
            ..Default::default()
        }))
        .map_err(|error| anyhow!("failed to configure tokenizer truncation: {error}"))?;
    Ok(tokenizer)
}

fn resolve_onnx_path(model_dir: &Path) -> Result<PathBuf> {
    let preferred = model_dir.join("onnx/model.onnx");
    if preferred.is_file() {
        return Ok(preferred);
    }
    let onnx_dir = model_dir.join("onnx");
    let mut entries = fs::read_dir(&onnx_dir)
        .with_context(|| format!("failed to read {}", onnx_dir.display()))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "onnx"))
        .collect::<Vec<_>>();
    entries.sort();
    entries.into_iter().next().ok_or_else(|| {
        anyhow!(
            "missing ONNX model file under {}",
            onnx_dir.display()
        )
    })
}

fn mean_pool_and_normalize(hidden: Array3<f32>, attention_mask: Array2<i64>) -> Result<Vec<f32>> {
    let hidden_dim = *hidden
        .shape()
        .get(2)
        .ok_or_else(|| anyhow!("unexpected hidden state shape"))?;
    if hidden_dim != EMBEDDING_DIM {
        bail!(
            "unexpected embedding dimension: expected {EMBEDDING_DIM}, got {hidden_dim}"
        );
    }
    let mut pooled = vec![0.0_f32; hidden_dim];
    let mut count = 0.0_f32;
    for (index, mask_value) in attention_mask.index_axis(Axis(0), 0).iter().enumerate() {
        if *mask_value == 0 {
            continue;
        }
        for dim in 0..hidden_dim {
            pooled[dim] += hidden[[0, index, dim]];
        }
        count += 1.0;
    }
    if count == 0.0 {
        bail!("attention mask removed every token");
    }
    for value in &mut pooled {
        *value /= count;
    }
    let norm = pooled.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        bail!("pooled embedding norm was zero");
    }
    Ok(pooled.into_iter().map(|value| value / norm).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn resolved_model_dir_prefers_env_override() {
        let original = env::var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR").ok();
        unsafe {
            env::set_var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR", "/tmp/e5-model");
        }
        assert_eq!(resolved_model_dir(), PathBuf::from("/tmp/e5-model"));
        match original {
            Some(value) => unsafe { env::set_var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR", value) },
            None => unsafe { env::remove_var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR") },
        }
    }

    #[test]
    fn mean_pool_and_normalize_returns_unit_vector() {
        let mut hidden = Array3::<f32>::zeros((1, 2, EMBEDDING_DIM));
        hidden[[0, 0, 0]] = 3.0;
        hidden[[0, 1, 0]] = 1.0;
        hidden[[0, 0, 1]] = 4.0;
        hidden[[0, 1, 1]] = 0.0;
        let mask = Array2::from_shape_vec((1, 2), vec![1_i64, 1_i64]).expect("shape");
        let embedding = mean_pool_and_normalize(hidden, mask).expect("pooling should succeed");
        let norm = embedding.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
        assert_eq!(embedding.len(), EMBEDDING_DIM);
    }
}
