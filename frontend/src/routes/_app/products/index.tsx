import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { FormMoney, FormSwitch, FormText } from '@/components/form/fields'
import { CategoryPicker } from '@/components/pickers/CategoryPicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  productsListOptions,
  useCreateProduct,
  useDisableProduct,
  useUpdateProduct,
  type ProductResponse,
} from '@/hooks/useProducts'
import {
  categoriesListOptions,
  useCreateCategory,
  useDisableCategory,
  useUpdateCategory,
  type ProductCategoryResponse,
} from '@/hooks/useCategories'
import { formatAmount } from '@/lib/money'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

const searchSchema = z.object({
  tab: z.enum(['products', 'categories']).default('products'),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/products/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'products:read'),
  staticData: { desktopOnly: true },
  component: ProductsPage,
})

function ProductsPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">产品与类别</h1>
      <Tabs
        value={search.tab}
        onValueChange={(value) => {
          void navigate({
            search: { tab: value as 'products' | 'categories', page_number: 1, page_size: 20 },
          })
        }}
      >
        <TabsList>
          <TabsTrigger value="products">产品</TabsTrigger>
          <TabsTrigger value="categories">类别</TabsTrigger>
        </TabsList>
      </Tabs>
      {search.tab === 'products' ? <ProductsTab /> : <CategoriesTab />}
    </div>
  )
}

/* ---------------- 产品 Tab ---------------- */

const productFormSchema = z.object({
  name: z.string().min(1, '请填写产品名称').max(128),
  category_id: z.string().min(1, '请选择类别'),
  series: z.string().optional(),
  brand_name: z.string().optional(),
  specification: z.string().optional(),
  unit: z.string().optional(),
  unit_price: z
    .string()
    .min(1, '请填写单价')
    .regex(/^\d{1,10}(\.\d{1,2})?$/, '单价格式:最多 10 位整数 + 2 位小数'),
})

type ProductFormValues = z.infer<typeof productFormSchema>

function ProductsTab() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(
    productsListOptions({ page_number: search.page_number, page_size: search.page_size }),
  )
  const [editing, setEditing] = useState<ProductResponse | 'new' | null>(null)
  const [disabling, setDisabling] = useState<ProductResponse | null>(null)
  const disableMutation = useDisableProduct()
  const rows = query.data?.items ?? []

  const columns: ColumnDef<ProductResponse>[] = [
    { accessorKey: 'name', header: '名称' },
    { accessorKey: 'category_name', header: '类别' },
    {
      accessorKey: 'unit_price',
      header: '单价',
      meta: { align: 'right' },
      cell: ({ row }) => (
        <span className="tabular-nums">{formatAmount(row.original.unit_price)}</span>
      ),
    },
    { accessorKey: 'specification', header: '规格', cell: ({ row }) => row.original.specification ?? '-' },
    { accessorKey: 'unit', header: '单位', cell: ({ row }) => row.original.unit ?? '-' },
    {
      accessorKey: 'status',
      header: '状态',
      cell: ({ row }) =>
        row.original.status === 'disabled' ? (
          <Badge variant="destructive">停用</Badge>
        ) : (
          <Badge variant="outline">启用</Badge>
        ),
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <Guard perm="products:write">
          <div className="flex justify-end gap-1">
            <Button variant="ghost" size="xs" onClick={() => setEditing(row.original)}>
              编辑
            </Button>
            {row.original.status === 'active' && (
              <Button variant="ghost" size="xs" onClick={() => setDisabling(row.original)}>
                停用
              </Button>
            )}
          </div>
        </Guard>
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-3">
      <DataTableToolbar
        exportConfig={{
          filename: '产品',
          columns: [
            { header: '名称', value: (r: ProductResponse) => r.name },
            { header: '类别', value: (r) => r.category_name },
            { header: '单价', value: (r) => formatAmount(r.unit_price) },
            { header: '状态', value: (r) => (r.status === 'active' ? '启用' : '停用') },
          ],
          rows,
        }}
        actions={
          <Guard perm="products:write">
            <Button onClick={() => setEditing('new')}>
              <Plus />
              新建产品
            </Button>
          </Guard>
        }
      />
      <DataTable
        tableId="products"
        columns={columns}
        data={rows}
        totalCount={query.data?.totalCount ?? 0}
        isLoading={query.isLoading}
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({ ...prev, page_number: page.pageNumber, page_size: page.pageSize }),
          })
        }}
      />
      {editing && (
        <ProductDialog product={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />
      )}
      <AlertDialog open={disabling !== null} onOpenChange={(open) => !open && setDisabling(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>停用产品 “{disabling?.name}”?</AlertDialogTitle>
            <AlertDialogDescription>停用后不可在销售录入中选择。</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!disabling) return
                disableMutation.mutate(disabling.id, {
                  onSuccess: (outcome) => notify.success(outcomeMessage(outcome, '已停用')),
                  onError: (error) => notify.error(error),
                })
                setDisabling(null)
              }}
            >
              确认停用
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

function ProductDialog({
  product,
  onClose,
}: {
  product: ProductResponse | null
  onClose: () => void
}) {
  const form = useForm<ProductFormValues>({
    resolver: zodResolver(productFormSchema),
    defaultValues: {
      name: product?.name ?? '',
      category_id: product?.category_id ?? '',
      series: product?.series ?? '',
      brand_name: product?.brand_name ?? '',
      specification: product?.specification ?? '',
      unit: product?.unit ?? '',
      unit_price: product?.unit_price ?? '',
    },
  })
  const createMutation = useCreateProduct()
  const updateMutation = useUpdateProduct()
  const isPending = createMutation.isPending || updateMutation.isPending

  const submit = form.handleSubmit((values) => {
    const body = {
      name: values.name,
      category_id: values.category_id,
      series: values.series || null,
      brand_name: values.brand_name || null,
      specification: values.specification || null,
      unit: values.unit || null,
      unit_price: values.unit_price,
    }
    const callbacks = {
      onSuccess: (outcome: Parameters<typeof outcomeMessage>[0]) => {
        notify.success(outcomeMessage(outcome, product ? '产品已更新' : '产品已创建'))
        onClose()
      },
      onError: (error: unknown) => notify.error(error),
    }
    if (product) {
      updateMutation.mutate({ productId: product.id, body }, callbacks)
    } else {
      createMutation.mutate({ ...body, status: 'active' }, callbacks)
    }
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{product ? '编辑产品' : '新建产品'}</DialogTitle>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={submit}>
          <FormText control={form.control} name="name" label="名称" required />
          <Field data-invalid={form.formState.errors.category_id ? true : undefined}>
            <FieldLabel>
              类别<span className="text-destructive">*</span>
            </FieldLabel>
            <CategoryPicker
              value={form.watch('category_id') || undefined}
              onChange={(v) => form.setValue('category_id', v ?? '', { shouldValidate: true })}
            />
            {form.formState.errors.category_id && (
              <FieldError>{form.formState.errors.category_id.message}</FieldError>
            )}
          </Field>
          <FormMoney control={form.control} name="unit_price" label="单价" required />
          <div className="grid grid-cols-2 gap-3">
            <FormText control={form.control} name="series" label="系列" />
            <FormText control={form.control} name="brand_name" label="品牌" />
            <FormText control={form.control} name="specification" label="规格" />
            <FormText control={form.control} name="unit" label="单位" />
          </div>
          <Button type="submit" disabled={isPending}>
            {isPending ? '保存中…' : '保存'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/* ---------------- 类别 Tab ---------------- */

const categoryFormSchema = z.object({
  category_name: z.string().min(1, '请填写类别名称').max(64),
  requires_operation_count: z.boolean(),
})

type CategoryFormValues = z.infer<typeof categoryFormSchema>

function CategoriesTab() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(
    categoriesListOptions({ page_number: search.page_number, page_size: search.page_size }),
  )
  const [editing, setEditing] = useState<ProductCategoryResponse | 'new' | null>(null)
  const disableMutation = useDisableCategory()
  const rows = query.data?.items ?? []

  const columns: ColumnDef<ProductCategoryResponse>[] = [
    { accessorKey: 'category_name', header: '类别名称' },
    {
      accessorKey: 'requires_operation_count',
      header: '需要次数账户',
      cell: ({ row }) => (row.original.requires_operation_count ? '是' : '否'),
    },
    {
      accessorKey: 'status',
      header: '状态',
      cell: ({ row }) =>
        row.original.status === 'disabled' ? (
          <Badge variant="destructive">停用</Badge>
        ) : (
          <Badge variant="outline">启用</Badge>
        ),
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <Guard perm="products:categories:write">
          <div className="flex justify-end gap-1">
            <Button variant="ghost" size="xs" onClick={() => setEditing(row.original)}>
              编辑
            </Button>
            {row.original.status === 'active' && (
              <Button
                variant="ghost"
                size="xs"
                onClick={() => {
                  disableMutation.mutate(row.original.id, {
                    onSuccess: (outcome) => notify.success(outcomeMessage(outcome, '已停用')),
                    onError: (error) => notify.error(error),
                  })
                }}
              >
                停用
              </Button>
            )}
          </div>
        </Guard>
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-3">
      <DataTableToolbar
        actions={
          <Guard perm="products:categories:write">
            <Button onClick={() => setEditing('new')}>
              <Plus />
              新建类别
            </Button>
          </Guard>
        }
      />
      <DataTable
        tableId="product-categories"
        columns={columns}
        data={rows}
        totalCount={query.data?.totalCount ?? 0}
        isLoading={query.isLoading}
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({ ...prev, page_number: page.pageNumber, page_size: page.pageSize }),
          })
        }}
      />
      {editing && (
        <CategoryDialog
          category={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  )
}

function CategoryDialog({
  category,
  onClose,
}: {
  category: ProductCategoryResponse | null
  onClose: () => void
}) {
  const form = useForm<CategoryFormValues>({
    resolver: zodResolver(categoryFormSchema),
    defaultValues: {
      category_name: category?.category_name ?? '',
      requires_operation_count: category?.requires_operation_count ?? false,
    },
  })
  const createMutation = useCreateCategory()
  const updateMutation = useUpdateCategory()
  const isPending = createMutation.isPending || updateMutation.isPending

  const submit = form.handleSubmit((values) => {
    const callbacks = {
      onSuccess: (outcome: Parameters<typeof outcomeMessage>[0]) => {
        notify.success(outcomeMessage(outcome, category ? '类别已更新' : '类别已创建'))
        onClose()
      },
      onError: (error: unknown) => notify.error(error),
    }
    if (category) {
      updateMutation.mutate({ categoryId: category.id, body: values }, callbacks)
    } else {
      createMutation.mutate({ ...values, status: 'active' }, callbacks)
    }
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{category ? '编辑类别' : '新建类别'}</DialogTitle>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={submit}>
          <FormText control={form.control} name="category_name" label="类别名称" required />
          <FormSwitch control={form.control} name="requires_operation_count" label="需要次数账户" />
          <p className="text-xs text-muted-foreground">
            开启后,该类别的销售记录必须填写可操作次数,并生成次数账户。
          </p>
          <Button type="submit" disabled={isPending}>
            {isPending ? '保存中…' : '保存'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
