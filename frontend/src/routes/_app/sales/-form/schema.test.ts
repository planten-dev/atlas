import { describe, expect, it } from 'vitest'
import { assembleDealRequest, assembleDebtCollectionRequest, salesFormSchema, SALES_FORM_DEFAULTS } from './schema'

const base={...SALES_FORM_DEFAULTS,customer_id:'c',record_date:'2026-07-24',handler_user_id:'u',total_amount:'100.00',received_amount:'40.00',lines:[{product_id:'p',item_name:'项目',requires_operation_count:false}],allocations:[{guide_user_id:'g',allocation_ratio:'100'}]}
describe('salesFormSchema',()=>{
  it('accepts partial-payment deal',()=>{expect(salesFormSchema.safeParse(base).success).toBe(true)})
  it('rejects deal received amount above total',()=>{expect(salesFormSchema.safeParse({...base,received_amount:'101.00'}).success).toBe(false)})
  it('accepts pre-service with zero received amount',()=>{expect(salesFormSchema.safeParse({...base,record_type:'pre_service',received_amount:'0',allocations:[]}).success).toBe(true)})
  it('rejects debt collection above customer balance',()=>{expect(salesFormSchema.safeParse({...base,record_type:'debt_collection',total_amount:'',lines:[],received_amount:'60.00',customer_outstanding:'50.00'}).success).toBe(false)})
  it('requires operation count for configured product',()=>{expect(salesFormSchema.safeParse({...base,lines:[{product_id:'p',item_name:'项目',requires_operation_count:true}]}).success).toBe(false)})
})
describe('request assembly',()=>{
  it('builds deal without internal fields',()=>{const r=assembleDealRequest(base);expect(r.lines[0]).not.toHaveProperty('requires_operation_count');expect(r.total_amount).toBe('100.00')})
  it('builds customer-level debt collection',()=>{const r=assembleDebtCollectionRequest({...base,record_type:'debt_collection',lines:[]});expect(r).not.toHaveProperty('lines');expect(r.customer_id).toBe('c')})
})
