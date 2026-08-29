import { LitElement, html, css } from 'lit'
import { customElement, property, state } from 'lit/decorators.js'
import type { Bookmark } from '../services/api.ts'

export interface BulkActionResult {
  success: boolean
  processed: number
  errors: number
  message: string
}

@customElement('bulk-actions')
export class BulkActions extends LitElement {
  @property({ type: Array }) selectedBookmarks: Bookmark[] = []
  @property({ type: Array }) availableTags: string[] = []
  @state() private _isProcessing = false
  @state() private _showTagInput = false
  @state() private _newTag = ''
  @state() private _selectedAction = ''

  static styles = css`
    :host {
      display: block;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      padding: 1rem;
      margin-bottom: 1rem;
      backdrop-filter: blur(10px);
      box-shadow: var(--shadow-sm);
      position: sticky;
      top: 1rem;
      z-index: 100;
    }

    .bulk-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 1rem;
    }

    .bulk-title {
      font-weight: bold;
      color: var(--text-primary);
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .selection-count {
      background: var(--accent-primary);
      color: var(--bg-primary);
      padding: 0.2rem 0.5rem;
      border-radius: 0.25rem;
      font-size: 0.75rem;
      font-weight: bold;
    }

    .clear-selection {
      background: none;
      border: none;
      color: var(--text-muted);
      cursor: pointer;
      padding: 0.25rem;
      border-radius: 0.25rem;
      transition: all 0.3s ease;
    }

    .clear-selection:hover {
      color: var(--accent-danger);
      background: rgba(var(--accent-danger), 0.1);
    }

    .bulk-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      align-items: center;
    }

    .action-button {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.5rem 1rem;
      border-radius: 0.25rem;
      font-size: 0.875rem;
      cursor: pointer;
      transition: all 0.3s ease;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .action-button:hover:not(:disabled) {
      border-color: var(--accent-primary);
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .action-button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }

    .action-button.danger:hover:not(:disabled) {
      border-color: var(--accent-danger);
      background: rgba(var(--accent-danger), 0.1);
      color: var(--accent-danger);
    }

    .tag-input-group {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-left: 0.5rem;
      padding-left: 0.5rem;
      border-left: 1px solid var(--border-color);
    }

    .tag-input {
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.5rem;
      border-radius: 0.25rem;
      font-size: 0.875rem;
      min-width: 120px;
    }

    .tag-input:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: 0 0 0 2px rgba(var(--accent-primary), 0.2);
    }

    .tag-suggestions {
      position: absolute;
      top: 100%;
      left: 0;
      right: 0;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.25rem;
      max-height: 150px;
      overflow-y: auto;
      z-index: 1000;
      box-shadow: var(--shadow-lg);
    }

    .tag-suggestion {
      padding: 0.5rem;
      cursor: pointer;
      border-bottom: 1px solid var(--border-color);
      transition: background-color 0.2s ease;
    }

    .tag-suggestion:hover {
      background: var(--bg-card-hover);
    }

    .tag-suggestion:last-child {
      border-bottom: none;
    }

    .spinner {
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

    @media (max-width: 768px) {
      .bulk-header {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
      }
      
      .bulk-actions {
        flex-direction: column;
        align-items: stretch;
      }
      
      .action-button {
        justify-content: center;
      }
      
      .tag-input-group {
        margin-left: 0;
        padding-left: 0;
        border-left: none;
        border-top: 1px solid var(--border-color);
        padding-top: 0.5rem;
      }
    }
  `

  render() {
    if (this.selectedBookmarks.length === 0) {
      return html``
    }

    return html`
      <div class="bulk-header">
        <div class="bulk-title">
          🎯 Bulk Actions
          <span class="selection-count">${this.selectedBookmarks.length} selected</span>
        </div>
        <button class="clear-selection" @click=${this._clearSelection} title="Clear selection">
          ✕ Clear
        </button>
      </div>

      <div class="bulk-actions">
        <button 
          class="action-button" 
          @click=${this._toggleFavorites}
          ?disabled=${this._isProcessing}>
          ${this._isProcessing && this._selectedAction === 'favorite' ? html`<span class="spinner"></span>` : '⭐'}
          Toggle Favorites
        </button>

        <button 
          class="action-button" 
          @click=${this._showTagInput ? this._hideTagInput : this._showTagInputField}
          ?disabled=${this._isProcessing}>
          ${this._showTagInput ? '🏷️ Cancel' : '🏷️ Add Tags'}
        </button>

        <button 
          class="action-button" 
          @click=${this._removeTags}
          ?disabled=${this._isProcessing}>
          ${this._isProcessing && this._selectedAction === 'remove-tags' ? html`<span class="spinner"></span>` : '🗑️'}
          Remove Tags
        </button>

        <button 
          class="action-button danger" 
          @click=${this._deleteBookmarks}
          ?disabled=${this._isProcessing}>
          ${this._isProcessing && this._selectedAction === 'delete' ? html`<span class="spinner"></span>` : '🗑️'}
          Delete All
        </button>

        ${this._showTagInput ? html`
          <div class="tag-input-group" style="position: relative;">
            <input 
              type="text" 
              class="tag-input" 
              placeholder="Enter tags (comma-separated)"
              .value=${this._newTag}
              @input=${this._handleTagInput}
              @keydown=${this._handleTagKeydown}>
            <button 
              class="action-button" 
              @click=${this._addTags}
              ?disabled=${!this._newTag.trim() || this._isProcessing}>
              ${this._isProcessing && this._selectedAction === 'add-tags' ? html`<span class="spinner"></span>` : '➕'}
              Add
            </button>
            ${this._renderTagSuggestions()}
          </div>
        ` : ''}
      </div>
    `
  }

  private _renderTagSuggestions() {
    if (!this._newTag.trim() || this.availableTags.length === 0) {
      return ''
    }

    const query = this._newTag.trim().toLowerCase()
    const suggestions = this.availableTags
      .filter(tag => tag.toLowerCase().includes(query) && tag.toLowerCase() !== query)
      .slice(0, 5)

    if (suggestions.length === 0) {
      return ''
    }

    return html`
      <div class="tag-suggestions">
        ${suggestions.map(tag => html`
          <div class="tag-suggestion" @click=${() => this._selectSuggestion(tag)}>
            ${tag}
          </div>
        `)}
      </div>
    `
  }

  private _clearSelection() {
    this.dispatchEvent(new CustomEvent('clear-selection'))
  }

  private _showTagInputField() {
    this._showTagInput = true
    this.requestUpdate()
    setTimeout(() => {
      const input = this.shadowRoot?.querySelector('.tag-input') as HTMLInputElement
      input?.focus()
    }, 100)
  }

  private _hideTagInput() {
    this._showTagInput = false
    this._newTag = ''
  }

  private _handleTagInput(e: Event) {
    const input = e.target as HTMLInputElement
    this._newTag = input.value
  }

  private _handleTagKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      this._addTags()
    } else if (e.key === 'Escape') {
      this._hideTagInput()
    }
  }

  private _selectSuggestion(tag: string) {
    this._newTag = tag
    this._addTags()
  }

  private async _toggleFavorites() {
    await this._performBulkAction('favorite', async () => {
      const promises = this.selectedBookmarks.map(bookmark => 
        fetch(`/api/bookmarks/${bookmark.id}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ is_favorite: !bookmark.is_favorite })
        })
      )
      await Promise.all(promises)
      return { processed: promises.length, errors: 0 }
    })
  }

  private async _addTags() {
    if (!this._newTag.trim()) return

    const tags = this._newTag.split(',').map(t => t.trim()).filter(t => t)
    
    await this._performBulkAction('add-tags', async () => {
      const promises = this.selectedBookmarks.map(bookmark => {
        const existingTags = bookmark.tags?.map(t => t.name) || []
        const newTags = [...new Set([...existingTags, ...tags])]
        
        return fetch(`/api/bookmarks/${bookmark.id}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ tags: newTags })
        })
      })
      await Promise.all(promises)
      return { processed: promises.length, errors: 0 }
    })

    this._hideTagInput()
  }

  private async _removeTags() {
    await this._performBulkAction('remove-tags', async () => {
      const promises = this.selectedBookmarks.map(bookmark => 
        fetch(`/api/bookmarks/${bookmark.id}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ tags: [] })
        })
      )
      await Promise.all(promises)
      return { processed: promises.length, errors: 0 }
    })
  }

  private async _deleteBookmarks() {
    if (!confirm(`Delete ${this.selectedBookmarks.length} bookmarks? This cannot be undone.`)) {
      return
    }

    await this._performBulkAction('delete', async () => {
      const promises = this.selectedBookmarks.map(bookmark => 
        fetch(`/api/bookmarks/${bookmark.id}`, { method: 'DELETE' })
      )
      await Promise.all(promises)
      return { processed: promises.length, errors: 0 }
    })
  }

  private async _performBulkAction(actionType: string, action: () => Promise<{processed: number, errors: number}>) {
    this._isProcessing = true
    this._selectedAction = actionType

    try {
      const result = await action()
      
      this.dispatchEvent(new CustomEvent('bulk-action-complete', {
        detail: {
          success: true,
          processed: result.processed,
          errors: result.errors,
          message: `Successfully processed ${result.processed} bookmarks`
        }
      }))
      
      this._clearSelection()
    } catch (error) {
      this.dispatchEvent(new CustomEvent('bulk-action-complete', {
        detail: {
          success: false,
          processed: 0,
          errors: this.selectedBookmarks.length,
          message: error instanceof Error ? error.message : 'Bulk action failed'
        }
      }))
    } finally {
      this._isProcessing = false
      this._selectedAction = ''
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'bulk-actions': BulkActions
  }
}