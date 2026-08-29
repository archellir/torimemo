import { LitElement, html, css } from 'lit'
import { customElement, state, property } from 'lit/decorators.js'
import { apiService, type Bookmark, type CreateBookmarkRequest, type UpdateBookmarkRequest } from '../services/api.ts'

@customElement('bookmark-dialog')
export class BookmarkDialog extends LitElement {
  @property({ type: Object }) editBookmark: Bookmark | null = null
  @state() private _url = ''
  @state() private _title = ''
  @state() private _description = ''
  @state() private _tags = ''
  @state() private _isAnalyzing = false
  @state() private _isSubmitting = false
  @state() private _error: string | null = null

  static styles = css`
    :host {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0, 0, 0, 0.8);
      backdrop-filter: blur(10px);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1000;
    }

    .dialog {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 1rem;
      padding: 2rem;
      width: 90%;
      max-width: 500px;
      max-height: 90vh;
      overflow-y: auto;
      box-shadow: var(--shadow-lg);
      position: relative;
    }

    .dialog:before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: linear-gradient(90deg, var(--accent-primary), var(--accent-secondary), var(--accent-warning));
    }

    .dialog-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 1.5rem;
    }

    .dialog-title {
      font-size: 1.25rem;
      font-weight: bold;
      color: var(--accent-primary);
      text-shadow: var(--shadow-sm);
    }

    .close-button {
      background: none;
      border: none;
      color: var(--text-muted);
      font-size: 1.5rem;
      cursor: pointer;
      padding: 0.25rem;
      border-radius: 0.25rem;
      transition: all 0.3s ease;
    }

    .close-button:hover {
      color: var(--accent-danger);
      background: rgba(var(--accent-danger), 0.1);
    }

    .error-message {
      background: rgba(var(--accent-danger), 0.1);
      border: 1px solid rgba(var(--accent-danger), 0.3);
      color: var(--accent-danger);
      padding: 1rem;
      border-radius: 0.5rem;
      margin-bottom: 1rem;
      text-align: center;
    }

    .form-group {
      margin-bottom: 1.5rem;
    }

    .form-label {
      display: block;
      margin-bottom: 0.5rem;
      color: var(--text-secondary);
      font-size: 0.875rem;
      font-weight: 500;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .form-input, .form-textarea {
      width: 100%;
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem;
      border-radius: 0.5rem;
      font-family: 'Courier New', monospace;
      transition: all 0.3s ease;
      box-sizing: border-box;
    }

    .form-input:focus, .form-textarea:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: var(--shadow-md);
    }

    .form-textarea {
      resize: vertical;
      min-height: 80px;
    }

    .form-input::placeholder, .form-textarea::placeholder {
      color: var(--text-muted);
    }

    .analyze-status {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-top: 0.5rem;
      color: var(--accent-primary);
      font-size: 0.875rem;
    }

    .analyze-spinner {
      width: 16px;
      height: 16px;
      border: 2px solid rgba(var(--accent-primary), 0.2);
      border-top: 2px solid var(--accent-primary);
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }

    @keyframes spin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }

    .tag-suggestions {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      margin-top: 0.5rem;
    }

    .tag-suggestion {
      background: rgba(var(--accent-secondary), 0.1);
      border: 1px solid rgba(var(--accent-secondary), 0.3);
      color: var(--accent-secondary);
      font-size: 0.7rem;
      padding: 0.25rem 0.5rem;
      border-radius: 0.25rem;
      cursor: pointer;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
    }

    .tag-suggestion:hover {
      background: rgba(var(--accent-secondary), 0.2);
      transform: translateY(-1px);
    }

    .tag-suggestion:before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
      transition: left 0.5s;
    }

    .tag-suggestion:hover:before {
      left: 100%;
    }

    .dialog-actions {
      display: flex;
      gap: 1rem;
      margin-top: 2rem;
    }

    .button {
      flex: 1;
      background: transparent;
      border: 1px solid;
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      font-family: 'Courier New', monospace;
      font-weight: bold;
      text-transform: uppercase;
      letter-spacing: 1px;
      cursor: pointer;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
    }

    .button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }

    .button-primary {
      border-color: var(--accent-primary);
      color: var(--accent-primary);
      background: var(--bg-card);
    }

    .button-primary:hover:not(:disabled) {
      background: var(--accent-primary);
      color: var(--bg-primary);
      box-shadow: var(--shadow-lg);
    }

    .button-secondary {
      border-color: var(--text-muted);
      color: var(--text-muted);
    }

    .button-secondary:hover:not(:disabled) {
      border-color: var(--text-secondary);
      color: var(--text-secondary);
    }

    .button:before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
      transition: left 0.5s;
    }

    .button:hover:before {
      left: 100%;
    }
  `

  connectedCallback() {
    super.connectedCallback()
    if (this.editBookmark) {
      this._populateForm()
    }
  }

  private _populateForm() {
    if (!this.editBookmark) return

    this._url = this.editBookmark.url
    this._title = this.editBookmark.title
    this._description = this.editBookmark.description || ''
    this._tags = this.editBookmark.tags?.map(tag => tag.name).join(', ') || ''
  }

  render() {
    const isEdit = !!this.editBookmark

    return html`
      <div class="dialog" @click=${this._handleDialogClick}>
        <div class="dialog-header">
          <h2 class="dialog-title">${isEdit ? 'Edit' : 'Add'} Bookmark</h2>
          <button class="close-button" @click=${this._handleClose}>×</button>
        </div>

        ${this._error ? html`
          <div class="error-message">
            ⚠️ ${this._error}
          </div>
        ` : ''}

        <form @submit=${this._handleSubmit}>
          <div class="form-group">
            <label class="form-label">URL</label>
            <input 
              type="url" 
              class="form-input"
              placeholder="https://example.com"
              .value=${this._url}
              @input=${this._handleUrlInput}
              required
            />
            ${this._isAnalyzing ? html`
              <div class="analyze-status">
                <div class="analyze-spinner"></div>
                AI is analyzing this link...
              </div>
            ` : ''}
          </div>

          <div class="form-group">
            <label class="form-label">Title</label>
            <input 
              type="text" 
              class="form-input"
              placeholder="Page title (auto-detected)"
              .value=${this._title}
              @input=${this._handleTitleInput}
            />
          </div>

          <div class="form-group">
            <label class="form-label">Description</label>
            <textarea 
              class="form-textarea"
              placeholder="Optional description..."
              .value=${this._description}
              @input=${this._handleDescriptionInput}
            ></textarea>
          </div>

          <div class="form-group">
            <label class="form-label">Tags</label>
            <input 
              type="text" 
              class="form-input"
              placeholder="development, react, tutorial (comma separated)"
              .value=${this._tags}
              @input=${this._handleTagsInput}
            />
            <div class="tag-suggestions">
              <span class="tag-suggestion" @click=${this._addSuggestedTag} data-tag="Development">Development</span>
              <span class="tag-suggestion" @click=${this._addSuggestedTag} data-tag="Tutorial">Tutorial</span>
              <span class="tag-suggestion" @click=${this._addSuggestedTag} data-tag="Reference">Reference</span>
              <span class="tag-suggestion" @click=${this._addSuggestedTag} data-tag="Code">Code</span>
            </div>
          </div>

          <div class="dialog-actions">
            <button type="button" class="button button-secondary" @click=${this._handleClose}>
              Cancel
            </button>
            <button type="submit" class="button button-primary" ?disabled=${this._isSubmitting}>
              ${this._isSubmitting ? 'Saving...' : `${isEdit ? 'Update' : 'Save'} Bookmark`}
            </button>
          </div>
        </form>
      </div>
    `
  }

  private _handleDialogClick(e: Event) {
    e.stopPropagation()
  }

  private _handleClose() {
    this.dispatchEvent(new CustomEvent('close'))
  }

  private async _handleSubmit(e: Event) {
    e.preventDefault()
    
    if (this._isSubmitting) return

    this._error = null
    this._isSubmitting = true

    try {
      const tags = this._tags.split(',').map(tag => tag.trim()).filter(tag => tag)
      const isEdit = !!this.editBookmark

      let bookmark: any

      if (isEdit) {
        const updateData: UpdateBookmarkRequest = {}
        
        if (this._url !== this.editBookmark!.url) updateData.url = this._url
        if (this._title !== this.editBookmark!.title) updateData.title = this._title
        if (this._description !== (this.editBookmark!.description || '')) {
          updateData.description = this._description || undefined
        }
        
        // Always update tags to handle additions/removals
        updateData.tags = tags

        bookmark = await apiService.updateBookmark(this.editBookmark!.id, updateData)
      } else {
        const createData: CreateBookmarkRequest = {
          url: this._url,
          title: this._title || this._url,
          description: this._description || undefined,
          tags
        }

        bookmark = await apiService.createBookmark(createData)
      }

      this.dispatchEvent(new CustomEvent('save', {
        detail: { bookmark, isEdit }
      }))
      
    } catch (error) {
      this._error = error instanceof Error ? error.message : 'Failed to save bookmark'
    } finally {
      this._isSubmitting = false
    }
  }

  private _handleUrlInput(e: Event) {
    const input = e.target as HTMLInputElement
    this._url = input.value
    
    // Simulate AI analysis for new bookmarks
    if (!this.editBookmark && this._url && this._url.startsWith('http')) {
      this._simulateAnalysis()
    }
  }

  private _handleTitleInput(e: Event) {
    const input = e.target as HTMLInputElement
    this._title = input.value
  }

  private _handleDescriptionInput(e: Event) {
    const textarea = e.target as HTMLTextAreaElement
    this._description = textarea.value
  }

  private _handleTagsInput(e: Event) {
    const input = e.target as HTMLInputElement
    this._tags = input.value
  }

  private _addSuggestedTag(e: Event) {
    const button = e.target as HTMLElement
    const tag = button.dataset.tag
    if (tag) {
      const currentTags = this._tags.split(',').map(t => t.trim()).filter(t => t)
      if (!currentTags.includes(tag)) {
        this._tags = [...currentTags, tag].join(', ')
      }
    }
  }

  private _simulateAnalysis() {
    this._isAnalyzing = true
    
    setTimeout(() => {
      this._isAnalyzing = false
      // Simulate fetched metadata based on URL
      if (this._url.includes('github.com')) {
        if (!this._title) this._title = 'GitHub Repository'
        this._tags = 'Development, Code, Git'
      } else if (this._url.includes('youtube.com')) {
        if (!this._title) this._title = 'YouTube Video'
        this._tags = 'Video, Tutorial, Media'
      } else if (this._url.includes('stackoverflow.com')) {
        if (!this._title) this._title = 'Stack Overflow Question'
        this._tags = 'Development, Q&A, Programming'
      } else {
        if (!this._title) this._title = 'Web Page'
        this._tags = 'Web, Reference'
      }
    }, 1500)
  }
}