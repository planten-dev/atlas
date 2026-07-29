import * as z from 'zod'
import { compareAmounts } from '@/lib/money'
import type { CreateDealRecordRequest, CreatePreServiceRecordRequest, CreateDebtCollectionRecordRequest } from '@/hooks/useSales'

const amount = z.string().regex(/^\d{1,10}(\.\d{1,2})?$/, '金额格式不正确')
const allocation = z.object({ guide_user_id: z.string().min(1, '请选择导购'), allocation_ratio: amount })
export function ratioToBasisPoints(value: string): number | null { const n=Number(value); return Number.isFinite(n)?Math.round(n*100):null }
export const salesLineSchema = z.object({ product_id: z.string().min(1, '请选择产品'), item_name: z.string().min(1, '请填写项目名称').max(128), operation_total_count: z.number().int().min(1).optional(), remark: z.string().max(2000).optional(), requires_operation_count: z.boolean() })
export const salesFormSchema = z.object({
  record_type: z.enum(['deal', 'pre_service', 'debt_collection']), customer_id: z.string().min(1, '请选择客户'), record_date: z.string().min(1, '请选择业务日期'),
  total_amount: z.string(), received_amount: z.string(), customer_outstanding: z.string().optional(),
  customer_type: z.enum(['new', 'returning']), deal_type: z.enum(['non_salon', 'salon']), handler_user_id: z.string().min(1, '请选择处理人'), expert_user_id: z.string().optional(), consultant_user_id: z.string().optional(), doctor_user_id: z.string().optional(), remark: z.string().max(2000).optional(), lines: z.array(salesLineSchema), allocations: z.array(allocation),
}).superRefine((v, ctx) => {
  if (v.record_type === 'debt_collection') {
    if (!amount.safeParse(v.received_amount).success || compareAmounts(v.received_amount || '0', '0') <= 0) ctx.addIssue({ code: 'custom', path: ['received_amount'], message: '收欠款金额必须大于 0' })
    if (v.customer_outstanding && compareAmounts(v.received_amount || '0', v.customer_outstanding) > 0) ctx.addIssue({ code: 'custom', path: ['received_amount'], message: '收欠款不能超过客户当前欠款' })
  } else {
    if (!amount.safeParse(v.total_amount).success || compareAmounts(v.total_amount || '0', '0') <= 0) ctx.addIssue({ code: 'custom', path: ['total_amount'], message: '销售总价必须大于 0' })
    if (v.lines.length === 0) ctx.addIssue({ code: 'custom', path: ['lines'], message: '至少一条售出内容' })
    v.lines.forEach((line, i) => { if (line.requires_operation_count && line.operation_total_count === undefined) ctx.addIssue({ code: 'custom', path: ['lines', i, 'operation_total_count'], message: '该商品需要填写可操作次数' }); if (!line.requires_operation_count && line.operation_total_count !== undefined) ctx.addIssue({ code: 'custom', path: ['lines', i, 'operation_total_count'], message: '该商品不可填写次数' }) })
    if (v.record_type === 'deal') { if (!amount.safeParse(v.received_amount).success || compareAmounts(v.received_amount || '0', '0') <= 0) ctx.addIssue({ code: 'custom', path: ['received_amount'], message: '本次实收必须大于 0' }); else if (compareAmounts(v.received_amount, v.total_amount) > 0) ctx.addIssue({ code: 'custom', path: ['received_amount'], message: '本次实收不能超过销售总价' }) }
  }
  if (v.record_type !== 'pre_service') {
    if (!v.allocations.length) ctx.addIssue({ code: 'custom', path: ['allocations'], message: '至少一条业绩分配' })
    const guides=v.allocations.map(a=>a.guide_user_id).filter(Boolean); if(new Set(guides).size!==guides.length)ctx.addIssue({code:'custom',path:['allocations'],message:'导购不能重复'})
    const total=v.allocations.reduce((n,a)=>n+Math.round(Number(a.allocation_ratio||0)*100),0); if(total!==10000)ctx.addIssue({code:'custom',path:['allocations'],message:'分配比例合计必须为 100%'})
  }
})
export type SalesFormValues=z.infer<typeof salesFormSchema>; export type SalesLineValues=z.infer<typeof salesLineSchema>
const staff=(v:SalesFormValues)=>({customer_id:v.customer_id,record_date:v.record_date,handler_user_id:v.handler_user_id,expert_user_id:v.expert_user_id||null,consultant_user_id:v.consultant_user_id||null,doctor_user_id:v.doctor_user_id||null,remark:v.remark||null})
const lines=(v:SalesFormValues)=>v.lines.map(l=>({product_id:l.product_id,item_name:l.item_name,operation_total_count:l.requires_operation_count?(l.operation_total_count??null):null,remark:l.remark||null}))
const allocations=(v:SalesFormValues)=>v.allocations.map(a=>({guide_user_id:a.guide_user_id,allocation_ratio:a.allocation_ratio}))
export const assembleDealRequest=(v:SalesFormValues):CreateDealRecordRequest=>({...staff(v),total_amount:v.total_amount,received_amount:v.received_amount,customer_type:v.customer_type,deal_type:v.deal_type,lines:lines(v),allocations:allocations(v)})
export const assemblePreServiceRequest=(v:SalesFormValues):CreatePreServiceRecordRequest=>({...staff(v),total_amount:v.total_amount,customer_type:v.customer_type,deal_type:v.deal_type,lines:lines(v)})
export const assembleDebtCollectionRequest=(v:SalesFormValues):CreateDebtCollectionRecordRequest=>({...staff(v),received_amount:v.received_amount,allocations:allocations(v)})
export const EMPTY_LINE:SalesLineValues={product_id:'',item_name:'',requires_operation_count:false}
export const SALES_FORM_DEFAULTS:SalesFormValues={record_type:'deal',customer_id:'',record_date:'',total_amount:'',received_amount:'',customer_outstanding:'',customer_type:'new',deal_type:'non_salon',handler_user_id:'',expert_user_id:'',consultant_user_id:'',doctor_user_id:'',remark:'',lines:[{...EMPTY_LINE}],allocations:[{guide_user_id:'',allocation_ratio:'100'}]}
