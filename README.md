# とりメモ (Torimemo) - Blazingly Fast Bookmark Manager

A lightning-fast, lightweight bookmark manager with AI-powered auto-categorization, built with Go + TypeScript + SQLite.

## Features

- **Blazingly Fast**: Single Go binary with embedded frontend
- **Ultra Lightweight**: <50MB RAM usage, minimal resource footprint
- **Full-Text Search**: Powered by SQLite FTS5 with ranked results
- **Smart Tagging**: Interactive tag cloud with auto-categorization
- **Cyberpunk UI**: Futuristic design with neon effects
- **Real-Time**: Instant search and updates
- **Container Ready**: Docker & Kubernetes deployment configs
- **AI Ready**: Learning system for intelligent categorization

## Quick Start

### Local Development

```bash
# Clone and build
git clone <repo>
cd bookmark.arcbjorn.com
./build.sh

# Run
./torimemo

# Open http://localhost:8080
```

### Docker

```bash
# Build and run
docker build -t torimemo .
docker run -p 8080:8080 -v torimemo_data:/data torimemo

# Or use docker-compose
docker-compose up -d
```

### Kubernetes

```bash
kubectl apply -f k8s.yaml
```

## API Documentation

### Bookmarks

- `GET /api/bookmarks` - List bookmarks (with pagination, filtering)
- `POST /api/bookmarks` - Create bookmark
- `GET /api/bookmarks/{id}` - Get bookmark
- `PUT /api/bookmarks/{id}` - Update bookmark
- `DELETE /api/bookmarks/{id}` - Delete bookmark
- `GET /api/bookmarks/search?q={query}` - Full-text search

### Tags

- `GET /api/tags` - List tags
- `GET /api/tags/cloud` - Tag cloud with sizes
- `GET /api/tags/popular` - Popular tags
- `POST /api/tags` - Create tag
- `PUT /api/tags/{id}` - Update tag
- `DELETE /api/tags/{id}` - Delete tag

### System

- `GET /api/health` - Health check
- `GET /api/stats` - Database statistics

## Architecture

- **Backend**: Go 1.25 (standard library only)
- **Database**: SQLite with FTS5, WAL mode
- **Frontend**: TypeScript + Lit Web Components + TailwindCSS
- **Search**: Full-text search with ranking and snippets
- **Tags**: Many-to-many with weighted tag cloud
- **Deployment**: Single binary with embedded assets

## Performance

- **Binary Size**: 12MB (with embedded frontend + AI)
- **Memory Usage**: <50MB RAM
- **Cold Start**: <100ms
- **Search Latency**: <10ms for 10k+ bookmarks
- **Database**: SQLite with FTS5 indexing

## Development

```bash
# Frontend development
cd web
pnpm install
pnpm run dev

# Backend development
go run main.go

# Build optimized
./build.sh
```

## Deployment Options

### Single Binary Deployment

The simplest deployment - just copy the binary:

```bash
# Build single binary (includes frontend)
./build.sh

# Deploy anywhere - single 12MB binary
scp torimemo user@server:/usr/local/bin/
ssh user@server 'torimemo'
```

### Production Deployment

Quick production setup with SSL and reverse proxy:

```bash
# Copy and edit environment variables
cp .env.example .env.production
# Edit .env.production with your values

# Deploy with SSL-enabled reverse proxy
./deploy.sh
```

### Environment Variables

- `PORT`: Server port (default: 8080)
- `DB_PATH`: Database path (default: ./torimemo.db)
- `LOG_LEVEL`: Logging level (DEBUG, INFO, WARN, ERROR)
- `ALLOWED_ORIGINS`: CORS allowed origins
- `RATE_LIMIT`: Rate limit (requests per minute)
- `RATE_BURST`: Rate limit burst size
- `JWT_SECRET`: JWT signing secret (change in production!)
- `JWT_EXPIRY`: JWT token expiry duration

### Docker Compose

```bash
# Development
docker-compose up -d

# Production with nginx reverse proxy
docker-compose -f docker-compose.prod.yml up -d
```

### Production Checklist

- [ ] Set strong `JWT_SECRET` in production
- [ ] Configure `ALLOWED_ORIGINS` for your domain
- [ ] Use real SSL certificates (replace self-signed)
- [ ] Set up database backups
- [ ] Configure log rotation
- [ ] Monitor health endpoint at `/api/health`
- [ ] Set appropriate `LOG_LEVEL` (WARN for production)

## AI Categorization

Three-layer AI system for intelligent bookmark categorization:

1. **Rule-based**: Domain patterns, URL analysis (always active)
2. **FastText**: Lightweight text classification (optional)
3. **ONNX Models**: Advanced content understanding (optional)

### Quick AI Setup

```bash
# Create model directories
mkdir -p ./models/fasttext ./models/onnx

# Start application (Layer 1 works immediately)
./torimemo

# Check AI predictive analysis
curl -X POST http://localhost:8080/api/ai/predict-tags \
 -H "Content-Type: application/json" \
 -d '{"url":"https://github.com","title":"GitHub","description":"Code repository"}'

# Find duplicate bookmarks
curl -X POST http://localhost:8080/api/ai/duplicates/check \
 -H "Content-Type: application/json" \
 -d '{"url":"https://github.com/golang/go","title":"Go Programming"}'
```

### AI API Endpoints

- `POST /api/ai/predict-tags` - Get predictive tag suggestions
- `POST /api/ai/duplicates/check` - Check for duplicate bookmarks
- `POST /api/ai/duplicates/find-all` - Find all duplicate groups
- `POST /api/ai/cluster` - Cluster bookmarks by similarity
- `POST /api/ai/cluster/analyze` - Analyze clustering potential
- `POST /api/ai/predict/learn` - Submit learning feedback

**Full AI Setup Guide**: See [AI_SETUP.md](AI_SETUP.md) for detailed model installation and configuration.

## License

MIT License - see LICENSE file for details.

---

Built with Go and TypeScript