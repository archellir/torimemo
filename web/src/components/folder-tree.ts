import { LitElement, html, css } from 'lit'
import { customElement, state, property } from 'lit/decorators.js'
import { apiService, type Folder, type FolderTree } from '../services/api.ts'

@customElement('folder-tree')
export class FolderTreeComponent extends LitElement {
  @property({ type: Boolean }) showBookmarkCounts = true
  @property({ type: Boolean }) allowSelection = false
  @property({ type: Number }) selectedFolderId = -1
  @state() private _folders: FolderTree[] = []
  @state() private _loading = false
  @state() private _expandedFolders: Set<number> = new Set()

  static styles = css`
    :host {
      display: block;
      font-family: 'Courier New', monospace;
    }

    .folder-tree {
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      overflow: hidden;
    }

    .folder-tree-header {
      padding: 1rem;
      background: var(--bg-secondary);
      border-bottom: 1px solid var(--border-color);
      font-weight: bold;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .folder-list {
      max-height: 400px;
      overflow-y: auto;
    }

    .folder-item {
      display: flex;
      align-items: center;
      padding: 0.5rem 1rem;
      cursor: pointer;
      transition: background-color 0.2s ease;
      border-bottom: 1px solid rgba(var(--border-color), 0.3);
    }

    .folder-item:last-child {
      border-bottom: none;
    }

    .folder-item:hover {
      background: var(--bg-card-hover);
    }

    .folder-item.selected {
      background: rgba(var(--accent-primary), 0.1);
      border-left: 3px solid var(--accent-primary);
    }

    .folder-indent {
      width: 1rem;
      flex-shrink: 0;
    }

    .folder-expand {
      width: 1rem;
      height: 1rem;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      font-size: 0.8rem;
      margin-right: 0.25rem;
      color: var(--text-muted);
    }

    .folder-expand:hover {
      color: var(--text-primary);
    }

    .folder-expand.expandable {
      background: rgba(var(--border-color), 0.2);
      border-radius: 0.2rem;
    }

    .folder-icon {
      margin-right: 0.5rem;
      font-size: 1rem;
      flex-shrink: 0;
    }

    .folder-name {
      flex: 1;
      font-size: 0.9rem;
      color: var(--text-primary);
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .folder-count {
      background: rgba(var(--text-muted), 0.2);
      color: var(--text-muted);
      font-size: 0.75rem;
      padding: 0.1rem 0.3rem;
      border-radius: 0.2rem;
      margin-left: 0.5rem;
    }

    .folder-actions {
      margin-left: 0.5rem;
      opacity: 0;
      transition: opacity 0.2s ease;
      display: flex;
      gap: 0.25rem;
    }

    .folder-item:hover .folder-actions {
      opacity: 1;
    }

    .folder-action {
      background: none;
      border: none;
      cursor: pointer;
      padding: 0.2rem;
      border-radius: 0.2rem;
      color: var(--text-muted);
      font-size: 0.8rem;
      transition: all 0.2s ease;
    }

    .folder-action:hover {
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .loading {
      padding: 2rem;
      text-align: center;
      color: var(--text-muted);
    }

    .empty {
      padding: 2rem;
      text-align: center;
      color: var(--text-muted);
      font-style: italic;
    }

    .create-folder-button {
      background: var(--accent-primary);
      border: none;
      color: var(--bg-primary);
      padding: 0.5rem 1rem;
      border-radius: 0.3rem;
      cursor: pointer;
      font-size: 0.8rem;
      font-weight: bold;
      margin: 0.5rem;
      transition: all 0.2s ease;
    }

    .create-folder-button:hover {
      background: var(--accent-secondary);
      transform: translateY(-1px);
    }

    .folder-path {
      font-size: 0.7rem;
      color: var(--text-muted);
      margin-top: 0.2rem;
      opacity: 0.8;
    }
  `

  connectedCallback() {
    super.connectedCallback()
    this._loadFolders()
  }

  render() {
    if (this._loading) {
      return html`
        <div class="folder-tree">
          <div class="folder-tree-header">
            📁 Folders
          </div>
          <div class="loading">Loading folders...</div>
        </div>
      `
    }

    return html`
      <div class="folder-tree">
        <div class="folder-tree-header">
          📁 Folders
          <button class="create-folder-button" @click=${this._createFolder}>
            + New
          </button>
        </div>
        
        <div class="folder-list">
          ${this._folders.length === 0 ? html`
            <div class="empty">No folders yet</div>
          ` : this._folders.map(tree => this._renderFolder(tree, 0))}
        </div>
      </div>
    `
  }

  private _renderFolder(tree: FolderTree, level: number): any {
    const folder = tree.folder
    const isExpanded = this._expandedFolders.has(folder.id)
    const hasChildren = tree.children && tree.children.length > 0
    const isSelected = this.allowSelection && this.selectedFolderId === folder.id

    return html`
      <div 
        class="folder-item ${isSelected ? 'selected' : ''}"
        @click=${() => this._selectFolder(folder)}>
        
        ${level > 0 ? html`<div class="folder-indent"></div>` : ''}
        
        <div 
          class="folder-expand ${hasChildren ? 'expandable' : ''}"
          @click=${(e: Event) => this._toggleFolder(e, folder.id)}>
          ${hasChildren ? (isExpanded ? '▼' : '▶') : ''}
        </div>

        <span class="folder-icon" style="color: ${folder.color}">
          ${folder.icon}
        </span>

        <div class="folder-content">
          <div class="folder-name">${folder.name}</div>
          ${folder.path !== folder.name ? html`
            <div class="folder-path">${folder.path}</div>
          ` : ''}
        </div>

        ${this.showBookmarkCounts && folder.bookmark_count > 0 ? html`
          <span class="folder-count">${folder.bookmark_count}</span>
        ` : ''}

        <div class="folder-actions">
          <button class="folder-action" @click=${(e: Event) => this._editFolder(e, folder)} title="Edit">
            ✏️
          </button>
          <button class="folder-action" @click=${(e: Event) => this._addSubfolder(e, folder)} title="Add subfolder">
            📁+
          </button>
          <button class="folder-action" @click=${(e: Event) => this._deleteFolder(e, folder)} title="Delete">
            🗑️
          </button>
        </div>
      </div>

      ${hasChildren && isExpanded ? tree.children.map(child => 
        this._renderFolder(child, level + 1)
      ) : ''}
    `
  }

  private async _loadFolders() {
    this._loading = true
    try {
      const response = await apiService.getFolderTree()
      this._folders = response.tree
    } catch (error) {
      console.error('Failed to load folders:', error)
    } finally {
      this._loading = false
    }
  }

  private _toggleFolder(e: Event, folderId: number) {
    e.stopPropagation()
    if (this._expandedFolders.has(folderId)) {
      this._expandedFolders.delete(folderId)
    } else {
      this._expandedFolders.add(folderId)
    }
    this.requestUpdate()
  }

  private _selectFolder(folder: Folder) {
    if (this.allowSelection) {
      this.selectedFolderId = folder.id
      this.dispatchEvent(new CustomEvent('folder-selected', {
        detail: { folder },
        bubbles: true
      }))
    }
  }

  private _createFolder() {
    this.dispatchEvent(new CustomEvent('folder-create', {
      bubbles: true
    }))
  }

  private _editFolder(e: Event, folder: Folder) {
    e.stopPropagation()
    this.dispatchEvent(new CustomEvent('folder-edit', {
      detail: { folder },
      bubbles: true
    }))
  }

  private _addSubfolder(e: Event, parentFolder: Folder) {
    e.stopPropagation()
    this.dispatchEvent(new CustomEvent('folder-create', {
      detail: { parentFolder },
      bubbles: true
    }))
  }

  private _deleteFolder(e: Event, folder: Folder) {
    e.stopPropagation()
    if (confirm(`Delete folder "${folder.name}" and all its subfolders?`)) {
      this.dispatchEvent(new CustomEvent('folder-delete', {
        detail: { folder },
        bubbles: true
      }))
    }
  }

  // Public method to refresh the folder tree
  async refresh() {
    await this._loadFolders()
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'folder-tree': FolderTreeComponent
  }
}