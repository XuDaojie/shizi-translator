import { cva, type VariantProps } from 'class-variance-authority'

export { default as Input } from './Input.vue'

export const inputVariants = cva(
  'flex w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm shadow-sm transition-colors duration-150 file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 disabled:cursor-not-allowed disabled:opacity-50',
  {
    variants: {
      size: {
        default: 'h-9',
        sm: 'h-8 text-xs',
        lg: 'h-10',
      },
      invalid: {
        true: 'border-destructive focus-visible:ring-destructive',
        false: '',
      },
    },
    defaultVariants: {
      size: 'default',
      invalid: false,
    },
  },
)

export type InputVariants = VariantProps<typeof inputVariants>
