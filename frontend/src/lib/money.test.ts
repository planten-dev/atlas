import { describe, expect, it } from 'vitest'
import {
  addAmounts,
  compareAmounts,
  formatAmount,
  fromCents,
  isValidAmount,
  sumAmounts,
  toCents,
} from '@/lib/money'

describe('money', () => {
  describe('isValidAmount', () => {
    it('接受合法金额', () => {
      expect(isValidAmount('0')).toBe(true)
      expect(isValidAmount('12.5')).toBe(true)
      expect(isValidAmount('12.50')).toBe(true)
      expect(isValidAmount('9999999999.99')).toBe(true)
    })
    it('拒绝非法输入', () => {
      expect(isValidAmount('')).toBe(false)
      expect(isValidAmount('-1')).toBe(false)
      expect(isValidAmount('1.234')).toBe(false)
      expect(isValidAmount('12345678901')).toBe(false) // 11 位整数超限
      expect(isValidAmount('abc')).toBe(false)
      expect(isValidAmount('1,000')).toBe(false)
    })
  })

  describe('toCents / fromCents', () => {
    it('往返一致且恒两位', () => {
      expect(fromCents(toCents('12.5'))).toBe('12.50')
      expect(fromCents(toCents('0'))).toBe('0.00')
      expect(fromCents(toCents('9999999999.99'))).toBe('9999999999.99')
    })
    it('非法输入抛错', () => {
      expect(() => toCents('1.234')).toThrow()
      expect(() => toCents('')).toThrow()
    })
    it('负分渲染负号(仅展示场景)', () => {
      expect(fromCents(-150n)).toBe('-1.50')
    })
  })

  describe('addAmounts', () => {
    it('无浮点漂移', () => {
      expect(addAmounts('0.1', '0.2')).toBe('0.30')
      expect(addAmounts('0.05', '0.05')).toBe('0.10')
    })
    it('进位正确', () => {
      expect(addAmounts('0.99', '0.01')).toBe('1.00')
      expect(addAmounts('9999999998.99', '1.00')).toBe('9999999999.99')
    })
  })

  describe('sumAmounts', () => {
    it('合计正确', () => {
      expect(sumAmounts(['1.10', '2.20', '3.30'])).toBe('6.60')
    })
    it('空数组为 0.00', () => {
      expect(sumAmounts([])).toBe('0.00')
    })
    it('跳过非法值不崩溃', () => {
      expect(sumAmounts(['1.00', 'bad', '2.00'])).toBe('3.00')
    })
  })

  describe('compareAmounts', () => {
    it('比较字符串金额', () => {
      expect(compareAmounts('2.00', '10.00')).toBe(-1) // 字符串比较会错,数值比较对
      expect(compareAmounts('1.50', '1.5')).toBe(0)
      expect(compareAmounts('3.00', '2.99')).toBe(1)
    })
  })

  describe('formatAmount', () => {
    it('归一两位小数', () => {
      expect(formatAmount('5')).toBe('5.00')
      expect(formatAmount('5.5')).toBe('5.50')
    })
    it('非法输入原样返回', () => {
      expect(formatAmount('n/a')).toBe('n/a')
    })
  })
})
