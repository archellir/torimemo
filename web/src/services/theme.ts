export type Theme = 'light' | 'dark' | 'auto';

export class ThemeManager {
  private static instance: ThemeManager;
  private currentTheme: Theme = 'dark'; // Default to dark (cyberpunk)
  private listeners: Set<(theme: Theme) => void> = new Set();

  private constructor() {
    this.loadTheme();
    this.applyTheme();
    this.setupSystemThemeListener();
  }

  static getInstance(): ThemeManager {
    if (!ThemeManager.instance) {
      ThemeManager.instance = new ThemeManager();
    }
    return ThemeManager.instance;
  }

  getCurrentTheme(): Theme {
    return this.currentTheme;
  }

  setTheme(theme: Theme): void {
    this.currentTheme = theme;
    this.saveTheme();
    this.applyTheme();
    this.notifyListeners();
  }

  subscribe(listener: (theme: Theme) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private loadTheme(): void {
    const saved = localStorage.getItem('torimemo-theme') as Theme;
    if (saved && ['light', 'dark', 'auto'].includes(saved)) {
      this.currentTheme = saved;
    }
  }

  private saveTheme(): void {
    localStorage.setItem('torimemo-theme', this.currentTheme);
  }

  private applyTheme(): void {
    const root = document.documentElement;
    const actualTheme = this.getActualTheme();
    
    // Remove existing theme classes
    root.classList.remove('light', 'dark');
    
    // Add current theme class
    root.classList.add(actualTheme);
    
    // Update CSS custom properties
    if (actualTheme === 'light') {
      this.applyLightTheme(root);
    } else {
      this.applyDarkTheme(root);
    }
  }

  private getActualTheme(): 'light' | 'dark' {
    if (this.currentTheme === 'auto') {
      return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    }
    return this.currentTheme;
  }

  private applyLightTheme(root: HTMLElement): void {
    root.style.setProperty('--bg-primary', '#ffffff');
    root.style.setProperty('--bg-secondary', '#f8f9fa');
    root.style.setProperty('--bg-tertiary', '#e9ecef');
    root.style.setProperty('--bg-card', '#ffffff');
    root.style.setProperty('--bg-card-hover', '#f8f9fa');
    
    root.style.setProperty('--text-primary', '#212529');
    root.style.setProperty('--text-secondary', '#6c757d');
    root.style.setProperty('--text-muted', '#adb5bd');
    
    root.style.setProperty('--border-color', '#dee2e6');
    root.style.setProperty('--border-focus', '#0d6efd');
    
    root.style.setProperty('--accent-primary', '#0d6efd');
    root.style.setProperty('--accent-secondary', '#6f42c1');
    root.style.setProperty('--accent-success', '#198754');
    root.style.setProperty('--accent-warning', '#fd7e14');
    root.style.setProperty('--accent-danger', '#dc3545');
    
    root.style.setProperty('--shadow-sm', '0 0.125rem 0.25rem rgba(0, 0, 0, 0.075)');
    root.style.setProperty('--shadow-md', '0 0.5rem 1rem rgba(0, 0, 0, 0.15)');
    root.style.setProperty('--shadow-lg', '0 1rem 3rem rgba(0, 0, 0, 0.175)');
  }

  private applyDarkTheme(root: HTMLElement): void {
    // Cyberpunk dark theme (existing colors)
    root.style.setProperty('--bg-primary', 'linear-gradient(135deg, #0a0a0a 0%, #1a1a1a 100%)');
    root.style.setProperty('--bg-secondary', 'rgba(0, 0, 0, 0.8)');
    root.style.setProperty('--bg-tertiary', 'rgba(0, 0, 0, 0.6)');
    root.style.setProperty('--bg-card', 'rgba(0, 0, 0, 0.8)');
    root.style.setProperty('--bg-card-hover', 'rgba(0, 255, 255, 0.05)');
    
    root.style.setProperty('--text-primary', '#00ffff');
    root.style.setProperty('--text-secondary', '#ffffff');
    root.style.setProperty('--text-muted', 'rgba(255, 255, 255, 0.6)');
    
    root.style.setProperty('--border-color', 'rgba(0, 255, 255, 0.3)');
    root.style.setProperty('--border-focus', '#00ffff');
    
    root.style.setProperty('--accent-primary', '#00ffff');
    root.style.setProperty('--accent-secondary', '#ff0080');
    root.style.setProperty('--accent-success', '#00ff00');
    root.style.setProperty('--accent-warning', '#ffff00');
    root.style.setProperty('--accent-danger', '#ff6b6b');
    
    root.style.setProperty('--shadow-sm', '0 0 10px rgba(0, 255, 255, 0.1)');
    root.style.setProperty('--shadow-md', '0 0 20px rgba(0, 255, 255, 0.2)');
    root.style.setProperty('--shadow-lg', '0 0 30px rgba(0, 255, 255, 0.3)');
  }

  private setupSystemThemeListener(): void {
    if (window.matchMedia) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: light)');
      mediaQuery.addListener(() => {
        if (this.currentTheme === 'auto') {
          this.applyTheme();
          this.notifyListeners();
        }
      });
    }
  }

  private notifyListeners(): void {
    this.listeners.forEach(listener => listener(this.currentTheme));
  }
}

// Global theme manager instance
export const themeManager = ThemeManager.getInstance();