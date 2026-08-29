import { LitElement, html, css } from 'lit'
import { customElement, state } from 'lit/decorators.js'

@customElement('bookmarklet-generator')
export class BookmarkletGenerator extends LitElement {
  @state() private _serverUrl = window.location.origin
  @state() private _showInstructions = false

  static styles = css`
    :host {
      display: block;
    }

    .bookmarklet-container {
      background: rgba(0, 255, 255, 0.05);
      border: 1px solid rgba(0, 255, 255, 0.2);
      border-radius: 0.5rem;
      padding: 1.5rem;
      margin: 1rem 0;
    }

    .bookmarklet-title {
      color: #00ffff;
      font-size: 1.1rem;
      font-weight: bold;
      margin-bottom: 1rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }

    .bookmarklet-title::before {
      content: '🔗';
      font-size: 1.2rem;
    }

    .bookmarklet-link {
      background: linear-gradient(45deg, rgba(255, 0, 128, 0.1), rgba(0, 255, 255, 0.1));
      border: 1px solid #ff0080;
      color: #ff0080;
      padding: 0.75rem 1rem;
      border-radius: 0.5rem;
      text-decoration: none;
      display: inline-block;
      font-family: 'Courier New', monospace;
      font-weight: bold;
      transition: all 0.3s ease;
      margin-bottom: 1rem;
      word-break: break-all;
    }

    .bookmarklet-link:hover {
      background: rgba(255, 0, 128, 0.2);
      box-shadow: 0 0 15px rgba(255, 0, 128, 0.3);
      transform: translateY(-1px);
    }

    .bookmarklet-link:active {
      transform: translateY(0);
    }

    .instructions-button {
      background: transparent;
      border: 1px solid #666;
      color: #666;
      padding: 0.5rem 1rem;
      border-radius: 0.25rem;
      cursor: pointer;
      font-size: 0.875rem;
      transition: all 0.3s ease;
      margin-left: 1rem;
    }

    .instructions-button:hover {
      border-color: #00ffff;
      color: #00ffff;
    }

    .instructions {
      background: rgba(0, 0, 0, 0.8);
      border: 1px solid rgba(255, 255, 0, 0.3);
      border-radius: 0.5rem;
      padding: 1rem;
      margin-top: 1rem;
      color: #ffff00;
      line-height: 1.6;
    }

    .instructions h4 {
      color: #ffff00;
      margin-bottom: 0.5rem;
      font-size: 0.95rem;
    }

    .instructions ol {
      margin: 0.5rem 0;
      padding-left: 1.5rem;
    }

    .instructions li {
      margin: 0.5rem 0;
      font-size: 0.875rem;
    }

    .instructions code {
      background: rgba(255, 255, 0, 0.1);
      padding: 0.2rem 0.4rem;
      border-radius: 0.25rem;
      font-family: 'Courier New', monospace;
    }

    .server-url-input {
      width: 200px;
      background: rgba(0, 0, 0, 0.8);
      border: 1px solid rgba(0, 255, 255, 0.3);
      color: white;
      padding: 0.5rem;
      border-radius: 0.25rem;
      font-family: 'Courier New', monospace;
      margin-left: 0.5rem;
    }

    .server-url-input:focus {
      outline: none;
      border-color: #00ffff;
      box-shadow: 0 0 10px rgba(0, 255, 255, 0.3);
    }

    .url-config {
      display: flex;
      align-items: center;
      margin-bottom: 1rem;
      font-size: 0.875rem;
      color: #a0a0a0;
    }

    @media (max-width: 768px) {
      .bookmarklet-link {
        font-size: 0.8rem;
        padding: 0.5rem;
      }
      
      .url-config {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
      }
      
      .server-url-input {
        width: 100%;
        margin-left: 0;
      }
    }
  `

  private get bookmarkletCode() {
    return `javascript:(function(){
      const url = encodeURIComponent(window.location.href);
      const title = encodeURIComponent(document.title);
      const description = encodeURIComponent(
        document.querySelector('meta[name="description"]')?.content || 
        document.querySelector('meta[property="og:description"]')?.content || 
        ''
      );
      const popup = window.open(
        '${this._serverUrl}/?add=' + url + '&title=' + title + '&desc=' + description,
        'torimemo-add',
        'width=500,height=600,scrollbars=yes,resizable=yes'
      );
      popup.focus();
    })();`
  }

  render() {
    return html`
      <div class="bookmarklet-container">
        <div class="bookmarklet-title">
          Quick Add Bookmarklet
          <button 
            class="instructions-button"
            @click=${this._toggleInstructions}>
            ${this._showInstructions ? 'Hide' : 'Show'} Instructions
          </button>
        </div>

        <div class="url-config">
          <span>Server URL:</span>
          <input 
            type="url" 
            class="server-url-input"
            .value=${this._serverUrl}
            @input=${this._handleUrlChange}
            placeholder="http://localhost:8080"
          />
        </div>

        <a 
          href="${this.bookmarkletCode}" 
          class="bookmarklet-link"
          @click=${this._handleBookmarkletClick}>
          📌 Add to Torimemo
        </a>

        ${this._showInstructions ? html`
          <div class="instructions">
            <h4>How to install:</h4>
            <ol>
              <li>Drag the "📌 Add to Torimemo" button above to your browser's bookmarks bar</li>
              <li>Or right-click the button and select "Bookmark this link"</li>
              <li>Name it something like "Add to Torimemo"</li>
            </ol>
            
            <h4>How to use:</h4>
            <ol>
              <li>Navigate to any webpage you want to bookmark</li>
              <li>Click the bookmarklet in your bookmarks bar</li>
              <li>A popup will open with the page title and URL pre-filled</li>
              <li>Add tags and description, then save!</li>
            </ol>

            <h4>Features:</h4>
            <ul>
              <li>Automatically extracts page title and description</li>
              <li>Works on any website</li>
              <li>Opens in a convenient popup window</li>
              <li>Supports custom server URLs</li>
            </ul>
          </div>
        ` : ''}
      </div>
    `
  }

  private _handleUrlChange(e: Event) {
    const input = e.target as HTMLInputElement
    this._serverUrl = input.value || window.location.origin
  }

  private _handleBookmarkletClick(e: Event) {
    e.preventDefault()
    
    // Test the bookmarklet on the current page
    const url = encodeURIComponent(window.location.href)
    const title = encodeURIComponent(document.title)
    const description = encodeURIComponent('Demo bookmark from bookmarklet generator')
    
    const popup = window.open(
      `${this._serverUrl}/?add=${url}&title=${title}&desc=${description}`,
      'torimemo-add',
      'width=500,height=600,scrollbars=yes,resizable=yes'
    )
    popup?.focus()
    
    // Show a message about dragging to bookmarks bar
    this.dispatchEvent(new CustomEvent('show-message', {
      detail: { 
        message: 'To install: drag this button to your bookmarks bar!',
        type: 'info'
      },
      bubbles: true
    }))
  }

  private _toggleInstructions() {
    this._showInstructions = !this._showInstructions
  }
}