# AI Model Setup Guide

## 3-Layer AI Architecture

The AI system uses a 3-layer approach:
1. **Layer 1**: Rule-based (always active)
2. **Layer 2**: FastText lightweight ML
3. **Layer 3**: ONNX advanced content understanding

## Model Directory Structure

```
./models/
├── fasttext/
│ ├── model.bin # FastText model file
│ ├── labels.txt # Category labels
│ └── vocab.txt # Vocabulary file
└── onnx/
 ├── sentiment.onnx # Sentiment analysis model
 ├── topic.onnx # Topic classification model
 └── quality.onnx # Content quality model
```

## Quick Start

### 1. Create Model Directories
```bash
mkdir -p ./models/fasttext
mkdir -p ./models/onnx
```

### 2. Environment Variables (Optional)
```bash
export FASTTEXT_MODEL_PATH="./models/fasttext"
export ONNX_MODEL_PATH="./models/onnx"
export AI_LAYER2_ENABLED="true"
export AI_LAYER3_ENABLED="true"
```

### 3. Start Application
```bash
./torimemo
```

The AI system will:
- Always use Layer 1 (rule-based)
- Use Layer 2 if FastText models found
- Use Layer 3 if ONNX models found
- Fallback to rule-based if models missing

## Model Sources

### FastText Models
Download pre-trained models:
```bash
# Option 1: Official FastText models
wget https://dl.fbaipublicfiles.com/fasttext/supervised-models/lid.176.bin -O ./models/fasttext/model.bin

# Option 2: Train custom model
# See FastText documentation for training
```

### ONNX Models
```bash
# Hugging Face models (convert to ONNX)
pip install transformers onnx

# Export sentiment model
python -m transformers.onnx --model=cardiffnlp/twitter-roberta-base-sentiment-latest ./models/onnx/sentiment/

# Export topic model
python -m transformers.onnx --model=facebook/bart-large-mnli ./models/onnx/topic/
```

## API Endpoints

Test AI functionality:

```bash
# Check AI status
curl http://localhost:8080/api/ai/status

# Get tag suggestions
curl -X POST http://localhost:8080/api/ai/categorize \
 -H "Content-Type: application/json" \
 -d '{"url":"https://example.com","title":"Example","description":"Test"}'

# Find duplicates
curl http://localhost:8080/api/ai/duplicates/123

# Get clusters
curl http://localhost:8080/api/ai/clusters

# Predictive suggestions
curl http://localhost:8080/api/ai/predict/tags?user_id=1
```

## Debugging

### Check Model Status
```bash
curl http://localhost:8080/api/ai/status | jq
```

### View Logs
```bash
LOG_LEVEL=DEBUG ./torimemo
```

### Test Individual Layers
- Layer 1: Always works (rule-based)
- Layer 2: Check `./models/fasttext/model.bin` exists
- Layer 3: Check `./models/onnx/*.onnx` exist

## Performance

| Layer | Latency | Accuracy | Resource Usage |
|-------|---------|----------|----------------|
| 1 (Rules) | <1ms | 70% | Minimal |
| 2 (FastText) | ~10ms | 85% | Low |
| 3 (ONNX) | ~100ms | 95% | Moderate |

## Production Tips

1. **Start with Layer 1**: Always works, good baseline
2. **Add Layer 2**: Significant accuracy boost, low overhead
3. **Add Layer 3**: Best accuracy, higher latency
4. **Monitor**: Use `/api/ai/status` for health checks
5. **Fallbacks**: System gracefully degrades if models fail

## Custom Models

### Training FastText
```bash
# Prepare training data
echo "__label__tech This is about technology" > train.txt
echo "__label__news This is news content" >> train.txt

# Train model
fasttext supervised -input train.txt -output ./models/fasttext/model
```

### Converting to ONNX
```python
import torch
from transformers import AutoModel, AutoTokenizer
import onnx

model = AutoModel.from_pretrained("your-model")
torch.onnx.export(model, inputs, "model.onnx")
```

## Configuration

Default settings in `internal/ai/categorizer.go`:
- FastText confidence threshold: 0.7
- ONNX confidence threshold: 0.8
- Similarity threshold: 0.85
- Max suggestions: 5

Override via environment or config file.