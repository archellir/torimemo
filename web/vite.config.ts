import { defineConfig } from 'vite'
import { resolve } from 'path'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
      '@components': resolve(__dirname, './src/components'),
      '@services': resolve(__dirname, './src/services'),
      '@styles': resolve(__dirname, './src/styles'),
      '~/types': resolve(__dirname, './src/types'),
    },
  },
  build: {
    // Production optimizations
    minify: 'terser',
    sourcemap: true,
    rollupOptions: {
      output: {
        // Better caching with content-based hashing
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
        // Code splitting for better caching
        manualChunks(id) {
          if (id.includes('node_modules')) {
            return 'vendor'
          }
          if (id.includes('src/services')) {
            return 'services'
          }
          if (id.includes('src/components')) {
            return 'components'
          }
        }
      }
    },
    // Performance optimization
    target: 'es2020',
    cssCodeSplit: true,
    // Report bundle size
    reportCompressedSize: true,
    chunkSizeWarningLimit: 500,
  },
  server: {
    // Development server optimization
    port: 5173,
    host: true,
    // API proxy for development
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        secure: false
      }
    }
  },
  preview: {
    port: 4173,
    host: true
  }
})