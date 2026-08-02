import vuePlugin from 'eslint-plugin-vue'
import vueParser from 'vue-eslint-parser'
import tseslint from '@typescript-eslint/eslint-plugin'
import tsparser from '@typescript-eslint/parser'

export default [
  {
    ignores: ['out/**', 'release/**', 'node_modules/**', 'dist/**']
  },
  {
    files: ['**/*.{ts,vue}'],
    languageOptions: {
      parser: tsparser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: 'module',
        extraFileExtensions: ['.vue']
      },
      globals: {
        window: 'readonly',
        document: 'readonly',
        console: 'readonly',
        navigator: 'readonly',
        location: 'readonly'
      }
    },
    plugins: {
      '@typescript-eslint': tseslint
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      '@typescript-eslint/no-explicit-any': 'off',
      'no-undef': 'off'
    }
  },
  {
    files: ['**/*.vue'],
    languageOptions: {
      parser: vueParser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: 'module',
        parser: tsparser,
        extraFileExtensions: ['.vue']
      }
    },
    plugins: {
      vue: vuePlugin
    },
    rules: {
      ...(vuePlugin.configs['flat/essential']?.rules ?? {})
    }
  }
]