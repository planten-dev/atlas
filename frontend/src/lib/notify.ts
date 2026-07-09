import { toast } from 'sonner'
import { ApiError } from '@/api/error'
import { errorMessage } from '@/lib/errors'

export const notify = {
  success(message: string) {
    toast.success(message)
  },
  info(message: string) {
    toast.info(message)
  },
  error(error: unknown) {
    if (error instanceof ApiError) {
      toast.error(errorMessage(error.code, error.message))
    } else if (error instanceof Error) {
      toast.error(error.message)
    } else {
      toast.error(String(error))
    }
  },
}
