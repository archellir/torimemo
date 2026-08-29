import { LitElement, html, css } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { themeManager, type Theme } from '../services/theme.js';

@customElement('theme-toggle')
export class ThemeToggle extends LitElement {
  @state()
  private currentTheme: Theme = 'dark';

  private unsubscribe?: () => void;

  static styles = css`
    :host {
      display: inline-block;
    }

    .theme-toggle {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.5rem;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      cursor: pointer;
      transition: all 0.3s ease;
      font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
      font-size: 0.875rem;
    }

    .theme-toggle:hover {
      background: var(--bg-card-hover);
      box-shadow: var(--shadow-md);
      border-color: var(--accent-primary);
    }

    .theme-icon {
      width: 1.25rem;
      height: 1.25rem;
      transition: transform 0.3s ease;
    }

    .theme-toggle:hover .theme-icon {
      transform: rotate(180deg);
    }

    .theme-select {
      display: none;
      position: absolute;
      top: 100%;
      left: 0;
      right: 0;
      background: var(--bg-card);
      border: 1px solid var(--border-color);
      border-radius: 0.5rem;
      box-shadow: var(--shadow-lg);
      z-index: 1000;
      margin-top: 0.25rem;
    }

    .theme-toggle.open .theme-select {
      display: block;
    }

    .theme-option {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.5rem 0.75rem;
      cursor: pointer;
      transition: background 0.2s ease;
      font-size: 0.875rem;
    }

    .theme-option:hover {
      background: var(--bg-card-hover);
    }

    .theme-option.active {
      background: rgba(var(--accent-primary), 0.1);
      color: var(--accent-primary);
    }

    .theme-option-icon {
      width: 1rem;
      height: 1rem;
    }

    .dropdown-container {
      position: relative;
    }

    /* Dark theme icons */
    :host(.dark) .sun-icon {
      display: none;
    }

    :host(.light) .moon-icon {
      display: none;
    }

    /* Animation effects */
    @keyframes glow {
      0%, 100% { 
        filter: drop-shadow(0 0 5px currentColor);
      }
      50% { 
        filter: drop-shadow(0 0 15px currentColor);
      }
    }

    .theme-icon {
      animation: glow 2s ease-in-out infinite;
    }

    /* Responsive design */
    @media (max-width: 768px) {
      .theme-toggle {
        padding: 0.375rem;
        font-size: 0.75rem;
      }

      .theme-icon {
        width: 1rem;
        height: 1rem;
      }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    this.currentTheme = themeManager.getCurrentTheme();
    this.unsubscribe = themeManager.subscribe((theme) => {
      this.currentTheme = theme;
      this.updateHostClass();
    });
    this.updateHostClass();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.unsubscribe?.();
  }

  private updateHostClass() {
    this.className = this.currentTheme === 'auto' 
      ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
      : this.currentTheme;
  }

  private toggleTheme() {
    const themes: Theme[] = ['dark', 'light', 'auto'];
    const currentIndex = themes.indexOf(this.currentTheme);
    const nextIndex = (currentIndex + 1) % themes.length;
    themeManager.setTheme(themes[nextIndex]);
  }

  // Public method for keyboard shortcuts
  public toggleThemeViaKeyboard() {
    this.toggleTheme();
  }

  private getThemeIcon() {
    switch (this.currentTheme) {
      case 'light':
        return this.getSunIcon();
      case 'dark':
        return this.getMoonIcon();
      case 'auto':
        return this.getAutoIcon();
      default:
        return this.getMoonIcon();
    }
  }

  private getSunIcon() {
    return html`
      <svg class="theme-icon sun-icon" fill="currentColor" viewBox="0 0 24 24">
        <path d="M12 2.25a.75.75 0 01.75.75v2.25a.75.75 0 01-1.5 0V3a.75.75 0 01.75-.75zM7.5 12a4.5 4.5 0 119 0 4.5 4.5 0 01-9 0zM18.894 6.166a.75.75 0 00-1.06-1.06l-1.591 1.59a.75.75 0 101.06 1.061l1.591-1.59zM21.75 12a.75.75 0 01-.75.75h-2.25a.75.75 0 010-1.5H21a.75.75 0 01.75.75zM17.834 18.894a.75.75 0 001.06-1.06l-1.59-1.591a.75.75 0 10-1.061 1.06l1.59 1.591zM12 18a.75.75 0 01.75.75V21a.75.75 0 01-1.5 0v-2.25A.75.75 0 0112 18zM7.758 17.303a.75.75 0 00-1.061-1.06l-1.591 1.59a.75.75 0 001.06 1.061l1.591-1.59zM6 12a.75.75 0 01-.75.75H3a.75.75 0 010-1.5h2.25A.75.75 0 016 12zM6.697 7.757a.75.75 0 001.06-1.06l-1.59-1.591a.75.75 0 00-1.061 1.06l1.59 1.591z" />
      </svg>
    `;
  }

  private getMoonIcon() {
    return html`
      <svg class="theme-icon moon-icon" fill="currentColor" viewBox="0 0 24 24">
        <path fill-rule="evenodd" d="M9.528 1.718a.75.75 0 01.162.819A8.97 8.97 0 009 6a9 9 0 009 9 8.97 8.97 0 003.463-.69.75.75 0 01.981.98 10.503 10.503 0 01-9.694 6.46c-5.799 0-10.5-4.701-10.5-10.5 0-4.368 2.667-8.112 6.46-9.694a.75.75 0 01.818.162z" clip-rule="evenodd" />
      </svg>
    `;
  }

  private getAutoIcon() {
    return html`
      <svg class="theme-icon" fill="currentColor" viewBox="0 0 24 24">
        <path d="M17.25 16.22a6.937 6.937 0 01-9.47-9.47 7.451 7.451 0 1013.04 13.04.75.75 0 01-1.06-1.06 6.937 6.937 0 01-2.51-2.51z" />
        <path fill-rule="evenodd" d="M.75 8.25a.75.75 0 01.75-.75h2.25a.75.75 0 010 1.5H1.5a.75.75 0 01-.75-.75zM4.5 5.5a.75.75 0 01.75-.75h2.25a.75.75 0 010 1.5H5.25A.75.75 0 014.5 5.5zM8.25 2.75a.75.75 0 01.75-.75h2.25a.75.75 0 010 1.5H9a.75.75 0 01-.75-.75z" clip-rule="evenodd" />
      </svg>
    `;
  }

  private getThemeLabel() {
    switch (this.currentTheme) {
      case 'light':
        return 'Light';
      case 'dark':
        return 'Dark';
      case 'auto':
        return 'Auto';
      default:
        return 'Dark';
    }
  }

  render() {
    return html`
      <div class="dropdown-container">
        <button 
          class="theme-toggle"
          @click=${this.toggleTheme}
          title="Switch theme (${this.getThemeLabel()})"
          aria-label="Switch theme"
        >
          ${this.getThemeIcon()}
          <span>${this.getThemeLabel()}</span>
        </button>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'theme-toggle': ThemeToggle;
  }
}