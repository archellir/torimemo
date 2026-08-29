import { LitElement, html, css } from 'lit'
import { customElement, property, state } from 'lit/decorators.js'
import { apiService, type ExtendedFolder } from '@services/api.ts'
import type { CreateFolderRequest, UpdateFolderRequest } from '~/types'

@customElement('folder-dialog')
export class FolderDialog extends LitElement {
  @property({ type: Boolean }) open = false
  @property({ type: Object }) folder: ExtendedFolder | null = null
  @property({ type: Object }) parentFolder: ExtendedFolder | null = null
  @state() private _name = ''
  @state() private _description = ''
  @state() private _color = '#666666'
  @state() private _icon = '📁'
  @state() private _loading = false
  @state() private _availableFolders: ExtendedFolder[] = []

  static styles = css`
    :host {
      position: fixed;
      top: 0;
      left: 0;
      width: 100vw;
      height: 100vh;
      background: rgba(0, 0, 0, 0.5);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1000;
      backdrop-filter: blur(5px);
    }

    :host(:not([open])) {
      display: none;
    }

    .dialog {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 1rem;
      width: 90%;
      max-width: 500px;
      max-height: 90vh;
      overflow-y: auto;
      box-shadow: var(--shadow-xl);
      animation: slideIn 0.3s ease;
    }

    @keyframes slideIn {
      from {
        opacity: 0;
        transform: translateY(-20px);
      }
      to {
        opacity: 1;
        transform: translateY(0);
      }
    }

    .dialog-header {
      padding: 1.5rem 1.5rem 1rem 1.5rem;
      border-bottom: 1px solid var(--border-color);
    }

    .dialog-title {
      font-size: 1.25rem;
      font-weight: bold;
      color: var(--text-primary);
      margin: 0;
    }

    .dialog-content {
      padding: 1.5rem;
    }

    .form-group {
      margin-bottom: 1.5rem;
    }

    .form-group:last-child {
      margin-bottom: 0;
    }

    .form-label {
      display: block;
      font-size: 0.9rem;
      font-weight: 500;
      color: var(--text-primary);
      margin-bottom: 0.5rem;
    }

    .form-input {
      width: 100%;
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem;
      border-radius: 0.5rem;
      font-family: inherit;
      font-size: 0.9rem;
      transition: all 0.3s ease;
    }

    .form-input:focus {
      outline: none;
      border-color: var(--accent-primary);
      box-shadow: 0 0 0 3px rgba(var(--accent-primary), 0.1);
    }

    .form-textarea {
      resize: vertical;
      min-height: 80px;
    }

    .color-icon-row {
      display: flex;
      gap: 1rem;
    }

    .color-input-group {
      flex: 1;
    }

    .icon-input-group {
      flex: 1;
    }

    .color-preview {
      width: 40px;
      height: 40px;
      border-radius: 0.5rem;
      border: 1px solid var(--border-color);
      margin-top: 0.5rem;
    }

    .icon-preview {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 1.5rem;
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      margin-top: 0.5rem;
      background: var(--bg-secondary);
    }

    .parent-select {
      width: 100%;
      background: var(--bg-primary);
      border: 1px solid var(--border-color);
      color: var(--text-primary);
      padding: 0.75rem;
      border-radius: 0.5rem;
      font-family: inherit;
      font-size: 0.9rem;
    }

    .dialog-actions {
      padding: 1rem 1.5rem 1.5rem 1.5rem;
      display: flex;
      gap: 1rem;
      justify-content: flex-end;
    }

    .btn {
      padding: 0.75rem 1.5rem;
      border-radius: 0.5rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.3s ease;
      font-family: inherit;
      font-size: 0.9rem;
      border: none;
    }

    .btn-primary {
      background: var(--accent-primary);
      color: var(--bg-primary);
    }

    .btn-primary:hover {
      background: var(--accent-secondary);
      transform: translateY(-1px);
    }

    .btn-primary:disabled {
      opacity: 0.6;
      cursor: not-allowed;
      transform: none;
    }

    .btn-secondary {
      background: transparent;
      color: var(--text-primary);
      border: 1px solid var(--border-color);
    }

    .btn-secondary:hover {
      background: var(--bg-card-hover);
    }

    .loading-spinner {
      display: inline-block;
      width: 16px;
      height: 16px;
      border: 2px solid transparent;
      border-top: 2px solid currentColor;
      border-radius: 50%;
      animation: spin 1s linear infinite;
      margin-right: 0.5rem;
    }

    @keyframes spin {
      to {
        transform: rotate(360deg);
      }
    }

    .icon-suggestions {
      display: flex;
      gap: 0.5rem;
      margin-top: 0.5rem;
      flex-wrap: wrap;
    }

    .icon-suggestion {
      background: var(--bg-secondary);
      border: 1px solid var(--border-color);
      border-radius: 0.3rem;
      padding: 0.3rem;
      cursor: pointer;
      font-size: 1.2rem;
      transition: all 0.2s ease;
    }

    .icon-suggestion:hover {
      background: var(--accent-primary);
      transform: scale(1.1);
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this._loadFolders()
  }

  updated(changedProperties: Map<string, any>) {
    if (changedProperties.has('folder')) {
      this._initializeForm()
    }
    if (changedProperties.has('parentFolder')) {
      this._initializeForm()
    }
    if (changedProperties.has('open') && this.open) {
      this._loadFolders()
    }
  }

  render() {
    const isEditing = this.folder !== null
    const title = isEditing ? 'Edit Folder' : 'Create Folder'

    return html`
      <div class="dialog" @click=${(e: Event) => e.stopPropagation()}>
        <div class="dialog-header">
          <h2 class="dialog-title">${title}</h2>
        </div>

        <div class="dialog-content">
          <form @submit=${this._handleSubmit}>
            <div class="form-group">
              <label class="form-label" for="name">Name *</label>
              <input
                type="text"
                id="name"
                class="form-input"
                .value=${this._name}
                @input=${(e: Event) => this._name = (e.target as HTMLInputElement).value}
                required
                maxlength="100"
                placeholder="Enter folder name">
            </div>

            <div class="form-group">
              <label class="form-label" for="description">Description</label>
              <textarea
                id="description"
                class="form-input form-textarea"
                .value=${this._description}
                @input=${(e: Event) => this._description = (e.target as HTMLTextAreaElement).value}
                placeholder="Optional description"
                rows="3"></textarea>
            </div>

            <div class="color-icon-row">
              <div class="color-input-group">
                <label class="form-label" for="color">Color</label>
                <input
                  type="color"
                  id="color"
                  class="form-input"
                  .value=${this._color}
                  @input=${(e: Event) => this._color = (e.target as HTMLInputElement).value}>
                <div class="color-preview" style="background-color: ${this._color}"></div>
              </div>

              <div class="icon-input-group">
                <label class="form-label" for="icon">Icon</label>
                <input
                  type="text"
                  id="icon"
                  class="form-input"
                  .value=${this._icon}
                  @input=${(e: Event) => this._icon = (e.target as HTMLInputElement).value}
                  maxlength="2"
                  placeholder="📁">
                <div class="icon-preview">${this._icon}</div>
              </div>
            </div>

            <div class="icon-suggestions">
              ${['📁', '📂', '💼', '🏠', '🎓', '🔧', '❤️', '⭐', '🏷️', '🗂️'].map(icon => html`
                <button type="button" class="icon-suggestion" @click=${() => this._icon = icon}>
                  ${icon}
                </button>
              `)}
            </div>

            <div class="form-group">
              <label class="form-label" for="parent">Parent Folder</label>
              <select
                id="parent"
                class="parent-select"
                @change=${(e: Event) => this._updateParent((e.target as HTMLSelectElement).value)}>
                <option value="">Root folder</option>
                ${this._availableFolders.map(folder => html`
                  <option 
                    value="${folder.id}"
                    ?selected=${this.parentFolder?.id === folder.id}>
                    ${folder.path}
                  </option>
                `)}
              </select>
            </div>
          </form>
        </div>

        <div class="dialog-actions">
          <button type="button" class="btn btn-secondary" @click=${this._close}>
            Cancel
          </button>
          <button 
            type="button" 
            class="btn btn-primary"
            @click=${this._handleSubmit}
            ?disabled=${this._loading || !this._name.trim()}>
            ${this._loading ? html`<span class="loading-spinner"></span>` : ''}
            ${isEditing ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    `
  }

  private async _loadFolders() {
    try {
      const response = await apiService.getFolders()
      // Filter out the current folder and its descendants when editing
      this._availableFolders = response.folders.filter(f => 
        !this.folder || (f.id !== this.folder.id && !f.path.startsWith(this.folder.path + '/'))
      )
    } catch (error) {
      console.error('Failed to load folders:', error)
    }
  }

  private _initializeForm() {
    if (this.folder) {
      // Editing existing folder
      this._name = this.folder.name
      this._description = this.folder.description || ''
      this._color = this.folder.color || '#666666'
      this._icon = this.folder.icon || '📁'
    } else {
      // Creating new folder
      this._name = ''
      this._description = ''
      this._color = '#666666'
      this._icon = '📁'
    }
  }

  private _updateParent(value: string) {
    if (value === '') {
      this.parentFolder = null
    } else {
      const parentId = parseInt(value)
      this.parentFolder = this._availableFolders.find(f => f.id === parentId) || null
    }
  }

  private async _handleSubmit(e?: Event) {
    if (e) e.preventDefault()

    if (!this._name.trim()) {
      return
    }

    this._loading = true
    
    try {
      if (this.folder) {
        // Update existing folder
        const updateData: UpdateFolderRequest = {
          name: this._name.trim(),
          description: this._description.trim() || undefined,
          color: this._color,
          icon: this._icon,
          parent_id: this.parentFolder?.id || undefined
        }

        const updated = await apiService.updateFolder(this.folder.id, updateData)
        this._dispatchSuccess('Folder updated successfully', updated)
      } else {
        // Create new folder
        const createData: CreateFolderRequest = {
          name: this._name.trim(),
          description: this._description.trim() || undefined,
          color: this._color,
          icon: this._icon,
          parent_id: this.parentFolder?.id || undefined
        }

        const created = await apiService.createFolder(createData)
        this._dispatchSuccess('Folder created successfully', created)
      }
      
      this._close()
    } catch (error) {
      console.error('Failed to save folder:', error)
      this._dispatchError(error instanceof Error ? error.message : 'Failed to save folder')
    } finally {
      this._loading = false
    }
  }

  private _close() {
    this.open = false
    this.dispatchEvent(new CustomEvent('dialog-close'))
  }

  private _dispatchSuccess(message: string, folder: ExtendedFolder) {
    this.dispatchEvent(new CustomEvent('folder-saved', {
      detail: { folder, message },
      bubbles: true
    }))
  }

  private _dispatchError(message: string) {
    this.dispatchEvent(new CustomEvent('error', {
      detail: { message },
      bubbles: true
    }))
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'folder-dialog': FolderDialog
  }
}