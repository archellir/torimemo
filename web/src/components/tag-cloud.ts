import { LitElement, html, css } from 'lit'
import { customElement, state, property } from 'lit/decorators.js'
import { apiService } from '../services/api.ts'

@customElement('tag-cloud')
export class TagCloud extends LitElement {
  @property() selectedTag = ''
  @state() private _tags: Array<{ name: string; count: number; size: number; color: string }> = []
  @state() private _loading = false
  @state() private _error: string | null = null
  @state() private _selectedTagIndex = -1

  static styles = css`
    :host {
      display: block;
    }

    .tag-cloud-container {
      min-height: 120px;
      position: relative;
    }

    .loading {
      display: flex;
      justify-content: center;
      align-items: center;
      padding: 2rem;
      color: var(--accent-primary);
    }

    .loading-spinner {
      width: 24px;
      height: 24px;
      border: 2px solid rgba(var(--accent-primary), 0.2);
      border-top: 2px solid var(--accent-primary);
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }

    @keyframes spin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }

    .error {
      color: var(--accent-danger);
      text-align: center;
      padding: 1rem;
      font-size: 0.875rem;
    }

    .tag-cloud {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      align-items: center;
      justify-content: flex-start;
      padding: 0.5rem;
    }

    .cloud-tag {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      color: var(--accent-primary);
      padding: 0.25rem 0.5rem;
      border-radius: 0.25rem;
      font-family: 'Courier New', monospace;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
      white-space: nowrap;
    }

    .cloud-tag.selected {
      background: rgba(var(--accent-primary), 0.2);
      border-color: var(--accent-primary);
      color: var(--text-primary);
      box-shadow: var(--shadow-md);
    }

    .cloud-tag:hover {
      background: var(--bg-card-hover);
      border-color: var(--accent-primary);
      transform: translateY(-1px);
    }

    .cloud-tag.keyboard-selected {
      background: var(--bg-card-hover);
      border-color: var(--accent-primary);
      transform: translateY(-1px);
      box-shadow: 0 0 0 1px var(--accent-primary);
    }

    :host(:focus) {
      outline: none;
    }

    .cloud-tag:before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.1), transparent);
      transition: left 0.5s;
    }

    .cloud-tag:hover:before {
      left: 100%;
    }

    .tag-count {
      opacity: 0.7;
      font-size: 0.8em;
      margin-left: 0.25rem;
    }

    .clear-filter {
      background: none;
      border: 1px solid var(--text-muted);
      color: var(--text-muted);
      padding: 0.25rem 0.5rem;
      border-radius: 0.25rem;
      font-size: 0.75rem;
      cursor: pointer;
      transition: all 0.3s ease;
    }

    .clear-filter:hover {
      border-color: var(--accent-danger);
      color: var(--accent-danger);
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this.loadTags()
    this._setupKeyboardNavigation()
  }

  disconnectedCallback() {
    super.disconnectedCallback()
    this.removeEventListener('keydown', this._handleKeydown)
  }

  private _setupKeyboardNavigation() {
    this.addEventListener('keydown', this._handleKeydown)
    this.tabIndex = 0
  }

  private _handleKeydown = (e: KeyboardEvent) => {
    if (this._tags.length === 0) return

    switch (e.key) {
      case 'ArrowRight':
      case 'l':
        e.preventDefault()
        this._selectedTagIndex = Math.min(this._selectedTagIndex + 1, this._tags.length - 1)
        break
      case 'ArrowLeft':
      case 'h':
        e.preventDefault()
        this._selectedTagIndex = Math.max(this._selectedTagIndex - 1, 0)
        break
      case 'Enter':
        e.preventDefault()
        if (this._selectedTagIndex >= 0) {
          this._selectTag(this._tags[this._selectedTagIndex].name)
        }
        break
      case 'Escape':
        e.preventDefault()
        this._clearFilter()
        break
    }
  }

  async loadTags() {
    this._loading = true
    this._error = null

    try {
      const response = await apiService.getTagCloud(20)
      this._tags = response.tags
    } catch (error) {
      this._error = error instanceof Error ? error.message : 'Failed to load tags'
    } finally {
      this._loading = false
    }
  }

  render() {
    if (this._loading) {
      return html`
        <div class="tag-cloud-container">
          <div class="loading">
            <div class="loading-spinner"></div>
          </div>
        </div>
      `
    }

    if (this._error) {
      return html`
        <div class="tag-cloud-container">
          <div class="error">
            Failed to load tags
          </div>
        </div>
      `
    }

    return html`
      <div class="tag-cloud-container">
        ${this.selectedTag ? html`
          <button class="clear-filter" @click=${this._clearFilter}>
            Clear filter ×
          </button>
        ` : ''}
        
        <div class="tag-cloud">
          ${this._tags.map((tag, index) => html`
            <button 
              class="cloud-tag ${this.selectedTag === tag.name ? 'selected' : ''} ${index === this._selectedTagIndex ? 'keyboard-selected' : ''}"
              style="font-size: ${0.7 + (tag.size * 0.4)}rem"
              @click=${() => this._selectTag(tag.name)}>
              ${tag.name}<span class="tag-count">${tag.count}</span>
            </button>
          `)}
        </div>
      </div>
    `
  }

  private _selectTag(tagName: string) {
    if (this.selectedTag === tagName) {
      this._clearFilter()
    } else {
      this.dispatchEvent(new CustomEvent('tag-selected', {
        detail: { tag: tagName }
      }))
    }
  }

  private _clearFilter() {
    this.dispatchEvent(new CustomEvent('tag-cleared'))
  }
}