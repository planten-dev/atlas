import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { productsListOptions, type ProductResponse } from '@/hooks/useProducts'

/** 产品选择器:服务端 keyword 搜索;onChange 回传完整产品供表单快照名称/次数规则。 */
export function ProductPicker({
  value,
  onChange,
  disabled,
  placeholder = '选择产品',
  className,
}: {
  value: string | undefined
  onChange: (productId: string | undefined, product?: ProductResponse) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const [keyword, setKeyword] = useState('')
  const { data, isLoading } = useQuery(
    productsListOptions({
      status_filter: 'active',
      keyword: keyword || undefined,
      page_size: 50,
    }),
  )
  const products = data?.items ?? []

  return (
    <PickerBase
      items={products.map((product) => ({
        value: product.id,
        label: product.name,
        keywords: [product.category_name, product.series ?? '', product.brand_name ?? ''].filter(
          Boolean,
        ),
      }))}
      value={value}
      onChange={(id) =>
        onChange(id, id ? products.find((product) => product.id === id) : undefined)
      }
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索产品名称/系列/品牌…"
      onSearchChange={setKeyword}
      className={className}
    />
  )
}
