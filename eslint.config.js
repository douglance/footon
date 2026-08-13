import eslint from '@eslint/js'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  {
    files: ['**/*.ts'],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      complexity: ['error', 8],
      'max-lines': ['error', { max: 220, skipBlankLines: true, skipComments: true }],
      'max-lines-per-function': ['error', { max: 45, skipBlankLines: true, skipComments: true }],
      'max-params': ['error', 4],
      'no-console': 'error',
    },
  },
  { ignores: ['.wrangler', 'dist', 'coverage', 'worker-configuration.d.ts', 'eslint.config.js'] },
)
