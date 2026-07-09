import { createFileRoute, Link } from '@tanstack/react-router'

export const Route = createFileRoute('/_app/403')({
  component: ForbiddenPage,
})

function ForbiddenPage() {
  return (
    <div className="flex flex-col items-center justify-center gap-4 py-24">
      <p className="text-5xl font-bold text-muted-foreground">403</p>
      <p className="text-muted-foreground">无权限访问该页面,请联系管理员</p>
      <Link to="/" className="text-primary underline underline-offset-4">
        回到工作台
      </Link>
    </div>
  )
}
