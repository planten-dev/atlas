use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::Decimal;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    dto::sales_records::{
        CreateCollectionPaymentRequest, CreateOperationUsageRequest, CreateSaleRecordRequest,
        CreateServiceRecordRequest, EnumParseError, ListOperationCountsQuery,
        ListOperationCountsResponse, ListOperationUsagesQuery, ListOperationUsagesResponse,
        ListSalesPaymentsQuery, ListSalesPaymentsResponse, ListSalesRecordsQuery,
        ListSalesRecordsResponse, OperationCountResponse, OperationUsageResponse, PatchField,
        SalesPaymentAllocationInput, SalesPaymentAllocationResponse, SalesPaymentInput,
        SalesPaymentResponse, SalesRecordLineInput, SalesRecordLineResponse, SalesRecordResponse,
        UpdateOperationCountRequest, UpdateOperationUsageRequest, format_money,
        parse_customer_type, parse_deal_type, parse_payment_type, parse_record_type, parse_status,
    },
    entities::{
        product_category, products as product_entity, sales_payments, sales_record_lines,
        sales_record_operation_counts, sales_records,
    },
    repositories::{
        RepositoryError,
        customers::CustomerRepository,
        product_categories::ProductCategoryRepository,
        products::ProductRepository,
        sales_records::{
            NewOperationCount, NewOperationUsage, NewSalesPayment, NewSalesPaymentAllocation,
            NewSalesRecord, NewSalesRecordLine, OperationCountChanges, OperationCountFilters,
            OperationUsageChanges, OperationUsageFilters, SalesPaymentFilters, SalesRecordFilters,
            SalesRecordRepository,
        },
        stores::StoreRepository,
        systems::SystemRepository,
        users::UserRepository,
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_ITEM_NAME_LENGTH: usize = 128;
const MAX_REMARK_LENGTH: usize = 2000;

#[derive(Clone)]
pub struct SalesRecordService {
    sales_records: SalesRecordRepository,
    customers: CustomerRepository,
    systems: SystemRepository,
    stores: StoreRepository,
    products: ProductRepository,
    categories: ProductCategoryRepository,
    users: UserRepository,
}

impl SalesRecordService {
    pub fn new(
        sales_records: SalesRecordRepository,
        customers: CustomerRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        categories: ProductCategoryRepository,
        users: UserRepository,
    ) -> Self {
        let products = ProductRepository::new(sales_records.db.clone());
        Self {
            sales_records,
            customers,
            systems,
            stores,
            products,
            categories,
            users,
        }
    }

    #[tracing::instrument(level = "info", skip(self, request), fields(customer_id = %request.customer_id))]
    pub async fn create_sale_record(
        &self,
        created_by_user_id: Uuid,
        request: CreateSaleRecordRequest,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        self.ensure_active_user("created_by_user_id", created_by_user_id)
            .await?;
        let scope = self.customer_scope(request.customer_id).await?;
        let customer_type =
            parse_customer_type("customer_type", &request.customer_type)?.to_string();
        let deal_type = parse_deal_type("deal_type", &request.deal_type)?.to_string();
        let remark = nullable_limited_text("remark", request.remark, MAX_REMARK_LENGTH)?;
        self.ensure_record_users(RecordUserInput {
            handler_user_id: request.handler_user_id,
            expert_user_id: request.expert_user_id,
            consultant_user_id: request.consultant_user_id,
            doctor_user_id: request.doctor_user_id,
        })
        .await?;

        let lines = self.prepare_lines("sale", request.lines).await?;
        let receivable_amount = active_receivable(&lines);
        if receivable_amount <= Decimal::ZERO {
            return Err(SalesRecordError::SaleReceivableRequired);
        }
        let payment_draft = self
            .prepare_payment(request.payment, receivable_amount)
            .await?;

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let record = self
            .sales_records
            .insert_sales_record(
                &tx,
                NewSalesRecord {
                    record_type: "sale".to_string(),
                    customer_id: request.customer_id,
                    record_date: request.record_date,
                    customer_type: Some(customer_type),
                    deal_type: Some(deal_type),
                    system_id: scope.system_id,
                    store_id: scope.store_id,
                    handler_user_id: request.handler_user_id,
                    expert_user_id: request.expert_user_id,
                    consultant_user_id: request.consultant_user_id,
                    doctor_user_id: request.doctor_user_id,
                    remark,
                    status: "active".to_string(),
                    created_by_user_id,
                },
                now,
            )
            .await?;

        for line in lines {
            let operation_total_count = line.operation_total_count;
            let line = self
                .sales_records
                .insert_sales_record_line(
                    &tx,
                    NewSalesRecordLine {
                        sales_record_id: record.id,
                        product_id: line.product_id,
                        item_name: line.item_name,
                        receivable_amount: line.receivable_amount,
                        operation_total_count,
                        remark: line.remark,
                        status: "active".to_string(),
                    },
                    now,
                )
                .await?;
            if let Some(total_count) = operation_total_count {
                self.sales_records
                    .insert_operation_count(
                        &tx,
                        NewOperationCount {
                            sales_record_line_id: line.id,
                            total_count,
                            used_count: 0,
                            status: "active".to_string(),
                        },
                        now,
                    )
                    .await?;
            }
        }

        let payment = self
            .sales_records
            .insert_sales_payment(
                &tx,
                NewSalesPayment {
                    sales_record_id: record.id,
                    payment_type: "initial".to_string(),
                    paid_amount: payment_draft.paid_amount,
                    paid_at: payment_draft.paid_at,
                    performance_status: "pending".to_string(),
                    status: "active".to_string(),
                    remark: payment_draft.remark,
                    created_by_user_id,
                },
                now,
            )
            .await?;
        self.insert_allocations(
            &tx,
            payment.id,
            payment.paid_amount,
            payment_draft.allocations,
            now,
        )
        .await?;

        let record_id = record.id;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%record_id, "created sale record through service");
        self.sales_record_detail(record_id).await
    }

    #[tracing::instrument(level = "info", skip(self, request), fields(customer_id = %request.customer_id))]
    pub async fn create_service_record(
        &self,
        created_by_user_id: Uuid,
        request: CreateServiceRecordRequest,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        self.ensure_active_user("created_by_user_id", created_by_user_id)
            .await?;
        let scope = self.customer_scope(request.customer_id).await?;
        let customer_type =
            optional_enum_text("customer_type", request.customer_type, parse_customer_type)?;
        let deal_type = optional_enum_text("deal_type", request.deal_type, parse_deal_type)?;
        let remark = nullable_limited_text("remark", request.remark, MAX_REMARK_LENGTH)?;
        self.ensure_record_users(RecordUserInput {
            handler_user_id: request.handler_user_id,
            expert_user_id: request.expert_user_id,
            consultant_user_id: request.consultant_user_id,
            doctor_user_id: request.doctor_user_id,
        })
        .await?;

        let lines = self.prepare_lines("service", request.lines).await?;

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let record = self
            .sales_records
            .insert_sales_record(
                &tx,
                NewSalesRecord {
                    record_type: "service".to_string(),
                    customer_id: request.customer_id,
                    record_date: request.record_date,
                    customer_type,
                    deal_type,
                    system_id: scope.system_id,
                    store_id: scope.store_id,
                    handler_user_id: request.handler_user_id,
                    expert_user_id: request.expert_user_id,
                    consultant_user_id: request.consultant_user_id,
                    doctor_user_id: request.doctor_user_id,
                    remark,
                    status: "active".to_string(),
                    created_by_user_id,
                },
                now,
            )
            .await?;

        for line in lines {
            self.sales_records
                .insert_sales_record_line(
                    &tx,
                    NewSalesRecordLine {
                        sales_record_id: record.id,
                        product_id: line.product_id,
                        item_name: line.item_name,
                        receivable_amount: line.receivable_amount,
                        operation_total_count: None,
                        remark: line.remark,
                        status: "active".to_string(),
                    },
                    now,
                )
                .await?;
        }

        let record_id = record.id;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%record_id, "created service record through service");
        self.sales_record_detail(record_id).await
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_sales_records(
        &self,
        query: ListSalesRecordsQuery,
    ) -> Result<ListSalesRecordsResponse, SalesRecordError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| parse_status("status_filter", value))
            .transpose()?;
        let record_type = query
            .record_type
            .as_deref()
            .map(|value| parse_record_type("record_type", value))
            .transpose()?;
        let (records, total_count) = self
            .sales_records
            .list_sales_records(
                SalesRecordFilters {
                    status_filter,
                    record_type,
                    customer_id: query.customer_id,
                    system_id: query.system_id,
                    store_id: query.store_id,
                    handler_user_id: query.handler_user_id,
                    record_date_from: query.record_date_from,
                    record_date_to: query.record_date_to,
                },
                page_number,
                page_size,
            )
            .await?;
        let sales_records = self.sales_record_responses(records).await?;

        Ok(ListSalesRecordsResponse {
            sales_records,
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn sales_record_detail(
        &self,
        sales_record_id: Uuid,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let record = self
            .sales_records
            .find_sales_record_by_id(sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        self.sales_record_response(record).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn void_sales_record(
        &self,
        sales_record_id: Uuid,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let record = self
            .sales_records
            .find_sales_record_by_id_for_update(&tx, sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            tx.commit().await.map_err(RepositoryError::from)?;
            return self.sales_record_detail(sales_record_id).await;
        }

        let lines = self
            .sales_records
            .find_lines_by_sales_record_id_in(&tx, sales_record_id)
            .await?;
        let line_ids = lines.iter().map(|line| line.id).collect::<Vec<_>>();
        self.ensure_no_active_usages(&tx, line_ids.clone()).await?;

        let record = self
            .sales_records
            .update_sales_record_status(&tx, &record, "voided", now)
            .await?;
        let counts = self
            .sales_records
            .find_operation_counts_by_line_ids_in(&tx, line_ids)
            .await?;
        for line in lines {
            if line.status != "voided" {
                self.sales_records
                    .update_line_status(&tx, &line, "voided", now)
                    .await?;
            }
        }
        for count in counts {
            if count.status != "voided" {
                self.sales_records
                    .update_operation_count(
                        &tx,
                        &count,
                        OperationCountChanges {
                            status: Some("voided".to_string()),
                            ..OperationCountChanges::default()
                        },
                        now,
                    )
                    .await?;
            }
        }
        for payment in self
            .sales_records
            .find_payments_by_sales_record_id_in(&tx, sales_record_id)
            .await?
        {
            if payment.status != "voided" {
                self.sales_records
                    .update_payment_status(&tx, &payment, "voided", now)
                    .await?;
            }
        }

        let record_id = record.id;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%record_id, "voided sales record through service");
        self.sales_record_detail(record_id).await
    }

    #[tracing::instrument(level = "info", skip(self, request), fields(sales_record_id = %request.sales_record_id))]
    pub async fn create_collection_payment(
        &self,
        created_by_user_id: Uuid,
        request: CreateCollectionPaymentRequest,
    ) -> Result<SalesPaymentResponse, SalesRecordError> {
        self.ensure_active_user("created_by_user_id", created_by_user_id)
            .await?;
        let paid_amount = parse_positive_money("paid_amount", request.paid_amount)?;
        let remark = nullable_limited_text("remark", request.remark, MAX_REMARK_LENGTH)?;
        let allocations = self
            .prepare_allocations(paid_amount, request.allocations)
            .await?;

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let record = self
            .sales_records
            .find_sales_record_by_id_for_update(&tx, request.sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            return Err(SalesRecordError::SalesRecordVoided);
        }
        if record.record_type != "sale" {
            return Err(SalesRecordError::CollectionRequiresSaleRecord);
        }

        let remaining = self
            .sales_record_remaining_amount_in(&tx, record.id)
            .await?;
        if paid_amount > remaining {
            return Err(SalesRecordError::PaymentExceedsOutstanding);
        }

        let payment = self
            .sales_records
            .insert_sales_payment(
                &tx,
                NewSalesPayment {
                    sales_record_id: record.id,
                    payment_type: "collection".to_string(),
                    paid_amount,
                    paid_at: request.paid_at,
                    performance_status: "pending".to_string(),
                    status: "active".to_string(),
                    remark,
                    created_by_user_id,
                },
                now,
            )
            .await?;
        self.insert_allocations(&tx, payment.id, paid_amount, allocations, now)
            .await?;

        let payment_id = payment.id;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%payment_id, "created collection payment through service");
        self.sales_payment_detail(payment_id).await
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_sales_payments(
        &self,
        query: ListSalesPaymentsQuery,
    ) -> Result<ListSalesPaymentsResponse, SalesRecordError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| parse_status("status_filter", value))
            .transpose()?;
        let payment_type = query
            .payment_type
            .as_deref()
            .map(|value| parse_payment_type("payment_type", value))
            .transpose()?;
        let (payments, total_count) = self
            .sales_records
            .list_sales_payments(
                SalesPaymentFilters {
                    status_filter,
                    payment_type,
                    sales_record_id: query.sales_record_id,
                    paid_at_from: query.paid_at_from,
                    paid_at_to: query.paid_at_to,
                },
                page_number,
                page_size,
            )
            .await?;
        let sales_payments = self.sales_payment_responses(payments).await?;

        Ok(ListSalesPaymentsResponse {
            sales_payments,
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn sales_payment_detail(
        &self,
        payment_id: Uuid,
    ) -> Result<SalesPaymentResponse, SalesRecordError> {
        let payment = self
            .sales_records
            .find_payment_by_id(payment_id)
            .await?
            .ok_or(SalesRecordError::SalesPaymentNotFound)?;
        let mut payments = self.sales_payment_responses(vec![payment]).await?;
        Ok(payments.remove(0))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn void_sales_payment(
        &self,
        payment_id: Uuid,
    ) -> Result<SalesPaymentResponse, SalesRecordError> {
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let payment = self
            .sales_records
            .find_payment_by_id_for_update(&tx, payment_id)
            .await?
            .ok_or(SalesRecordError::SalesPaymentNotFound)?;
        if payment.status == "voided" {
            tx.commit().await.map_err(RepositoryError::from)?;
            return self.sales_payment_detail(payment_id).await;
        }
        let payment = self
            .sales_records
            .update_payment_status(&tx, &payment, "voided", now)
            .await?;
        let payment_id = payment.id;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%payment_id, "voided sales payment through service");
        self.sales_payment_detail(payment_id).await
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_operation_counts(
        &self,
        query: ListOperationCountsQuery,
    ) -> Result<ListOperationCountsResponse, SalesRecordError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| parse_status("status_filter", value))
            .transpose()?;
        let (counts, total_count) = self
            .sales_records
            .list_operation_counts(
                OperationCountFilters {
                    status_filter,
                    sales_record_line_id: query.sales_record_line_id,
                    sales_record_id: query.sales_record_id,
                },
                page_number,
                page_size,
            )
            .await?;

        let record_ids = self
            .record_ids_by_line_ids(counts.iter().map(|count| count.sales_record_line_id))
            .await?;
        let operation_counts = counts
            .into_iter()
            .map(|count| {
                let record_id = record_ids
                    .get(&count.sales_record_line_id)
                    .copied()
                    .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
                Ok(OperationCountResponse::from_model(count, record_id))
            })
            .collect::<Result<Vec<_>, SalesRecordError>>()?;

        Ok(ListOperationCountsResponse {
            operation_counts,
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn operation_count_detail(
        &self,
        sales_record_line_id: Uuid,
    ) -> Result<OperationCountResponse, SalesRecordError> {
        let count = self
            .sales_records
            .find_operation_count(sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::OperationCountNotFound)?;
        let record_id = self.record_id_for_line(sales_record_line_id).await?;
        Ok(OperationCountResponse::from_model(count, record_id))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_operation_count(
        &self,
        sales_record_line_id: Uuid,
        request: UpdateOperationCountRequest,
    ) -> Result<OperationCountResponse, SalesRecordError> {
        validate_positive_count("total_count", request.total_count)?;
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let line = self
            .sales_records
            .find_line_by_id_for_update(&tx, sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
        self.ensure_active_line_record(&tx, &line).await?;
        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::OperationCountNotFound)?;
        if count.status == "voided" {
            return Err(SalesRecordError::OperationCountVoided);
        }
        if request.total_count < count.used_count {
            return Err(SalesRecordError::OperationCountBelowUsed);
        }
        let count = self
            .sales_records
            .update_operation_count(
                &tx,
                &count,
                OperationCountChanges {
                    total_count: Some(request.total_count),
                    ..OperationCountChanges::default()
                },
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(OperationCountResponse::from_model(
            count,
            line.sales_record_id,
        ))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_operation_usages(
        &self,
        query: ListOperationUsagesQuery,
    ) -> Result<ListOperationUsagesResponse, SalesRecordError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| parse_status("status_filter", value))
            .transpose()?;
        let (usages, total_count) = self
            .sales_records
            .list_operation_usages(
                OperationUsageFilters {
                    status_filter,
                    sales_record_line_id: query.sales_record_line_id,
                    sales_record_id: query.sales_record_id,
                    operator_user_id: query.operator_user_id,
                    doctor_user_id: query.doctor_user_id,
                    operated_at_from: query.operated_at_from,
                    operated_at_to: query.operated_at_to,
                },
                page_number,
                page_size,
            )
            .await?;

        let record_ids = self
            .record_ids_by_line_ids(usages.iter().map(|usage| usage.sales_record_line_id))
            .await?;
        let operation_usages = usages
            .into_iter()
            .map(|usage| {
                let record_id = record_ids
                    .get(&usage.sales_record_line_id)
                    .copied()
                    .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
                Ok(OperationUsageResponse::from_model(usage, record_id))
            })
            .collect::<Result<Vec<_>, SalesRecordError>>()?;

        Ok(ListOperationUsagesResponse {
            operation_usages,
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn operation_usage_detail(
        &self,
        usage_id: Uuid,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        let usage = self
            .sales_records
            .find_operation_usage_by_id(usage_id)
            .await?
            .ok_or(SalesRecordError::OperationUsageNotFound)?;
        let record_id = self.record_id_for_line(usage.sales_record_line_id).await?;
        Ok(OperationUsageResponse::from_model(usage, record_id))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_operation_usage(
        &self,
        request: CreateOperationUsageRequest,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        validate_positive_count("operation_count", request.operation_count)?;
        self.ensure_active_user("operator_user_id", request.operator_user_id)
            .await?;
        self.ensure_optional_active_user("doctor_user_id", request.doctor_user_id)
            .await?;
        let remark = nullable_limited_text("remark", request.remark, MAX_REMARK_LENGTH)?;

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let line = self
            .sales_records
            .find_line_by_id_for_update(&tx, request.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
        self.ensure_active_line_record(&tx, &line).await?;
        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, request.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::OperationCountNotFound)?;
        let count = self
            .apply_usage_delta(&tx, &count, request.operation_count, now)
            .await?;
        let usage = self
            .sales_records
            .insert_operation_usage(
                &tx,
                NewOperationUsage {
                    sales_record_line_id: request.sales_record_line_id,
                    operated_at: request.operated_at,
                    operator_user_id: request.operator_user_id,
                    doctor_user_id: request.doctor_user_id,
                    operation_count: request.operation_count,
                    remark,
                    status: "active".to_string(),
                },
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(
            operation_usage_id = %usage.id,
            sales_record_line_id = %usage.sales_record_line_id,
            used_count = count.used_count,
            "created operation usage through service"
        );
        Ok(OperationUsageResponse::from_model(
            usage,
            line.sales_record_id,
        ))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_operation_usage(
        &self,
        usage_id: Uuid,
        request: UpdateOperationUsageRequest,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        let changes = operation_usage_changes(request)?;
        if let Some(operator_user_id) = changes.operator_user_id {
            self.ensure_active_user("operator_user_id", operator_user_id)
                .await?;
        }
        if let Some(doctor_user_id) = changes.doctor_user_id {
            self.ensure_optional_active_user("doctor_user_id", doctor_user_id)
                .await?;
        }

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let usage = self
            .sales_records
            .find_operation_usage_by_id_for_update(&tx, usage_id)
            .await?
            .ok_or(SalesRecordError::OperationUsageNotFound)?;
        if usage.status == "voided" {
            return Err(SalesRecordError::OperationUsageVoided);
        }
        if changes.is_empty() {
            tx.commit().await.map_err(RepositoryError::from)?;
            let record_id = self.record_id_for_line(usage.sales_record_line_id).await?;
            return Ok(OperationUsageResponse::from_model(usage, record_id));
        }
        let line = self
            .sales_records
            .find_line_by_id_for_update(&tx, usage.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
        self.ensure_active_line_record(&tx, &line).await?;
        if let Some(new_count) = changes.operation_count {
            let delta = new_count - usage.operation_count;
            if delta != 0 {
                let count = self
                    .sales_records
                    .find_operation_count_for_update(&tx, usage.sales_record_line_id)
                    .await?
                    .ok_or(SalesRecordError::OperationCountNotFound)?;
                self.apply_usage_delta(&tx, &count, delta, now).await?;
            }
        }
        let usage = self
            .sales_records
            .update_operation_usage(&tx, &usage, changes, now)
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(OperationUsageResponse::from_model(
            usage,
            line.sales_record_id,
        ))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn void_operation_usage(
        &self,
        usage_id: Uuid,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let usage = self
            .sales_records
            .find_operation_usage_by_id_for_update(&tx, usage_id)
            .await?
            .ok_or(SalesRecordError::OperationUsageNotFound)?;
        if usage.status == "voided" {
            tx.commit().await.map_err(RepositoryError::from)?;
            let record_id = self.record_id_for_line(usage.sales_record_line_id).await?;
            return Ok(OperationUsageResponse::from_model(usage, record_id));
        }
        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, usage.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::OperationCountNotFound)?;
        self.apply_usage_delta(&tx, &count, -usage.operation_count, now)
            .await?;
        let usage = self
            .sales_records
            .update_operation_usage(
                &tx,
                &usage,
                OperationUsageChanges {
                    status: Some("voided".to_string()),
                    ..OperationUsageChanges::default()
                },
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        let record_id = self.record_id_for_line(usage.sales_record_line_id).await?;
        Ok(OperationUsageResponse::from_model(usage, record_id))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_operation_usage(&self, usage_id: Uuid) -> Result<(), SalesRecordError> {
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let usage = self
            .sales_records
            .find_operation_usage_by_id_for_update(&tx, usage_id)
            .await?
            .ok_or(SalesRecordError::OperationUsageNotFound)?;
        if usage.status == "active" {
            let count = self
                .sales_records
                .find_operation_count_for_update(&tx, usage.sales_record_line_id)
                .await?
                .ok_or(SalesRecordError::OperationCountNotFound)?;
            self.apply_usage_delta(&tx, &count, -usage.operation_count, now)
                .await?;
        }
        let deleted = self
            .sales_records
            .delete_operation_usage_by_id(&tx, usage_id)
            .await?;
        if !deleted {
            return Err(SalesRecordError::OperationUsageNotFound);
        }
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(())
    }

    async fn prepare_lines(
        &self,
        record_type: &'static str,
        request_lines: Vec<SalesRecordLineInput>,
    ) -> Result<Vec<LineDraft>, SalesRecordError> {
        if request_lines.is_empty() {
            return Err(SalesRecordError::SalesRecordLinesRequired);
        }

        let mut lines = Vec::with_capacity(request_lines.len());
        for line in request_lines {
            let product = self.ensure_active_product(line.product_id).await?;
            let category = self.ensure_active_category(product.category_id).await?;
            let item_name =
                required_limited_text("item_name", line.item_name, MAX_ITEM_NAME_LENGTH)?;
            let receivable_amount = parse_money("receivable_amount", Some(line.receivable_amount))?;
            let remark = nullable_limited_text("remark", line.remark, MAX_REMARK_LENGTH)?;
            let operation_total_count = match record_type {
                "sale" => self.validate_sale_operation_count(
                    category.requires_operation_count,
                    line.operation_total_count,
                )?,
                "service" => {
                    if receivable_amount != Decimal::ZERO {
                        return Err(SalesRecordError::ServiceReceivableMustBeZero);
                    }
                    if line.operation_total_count.is_some() {
                        return Err(SalesRecordError::ServiceOperationCountNotAllowed);
                    }
                    None
                }
                _ => return Err(SalesRecordError::InvalidRecordTypeInternal),
            };

            lines.push(LineDraft {
                product_id: product.id,
                item_name,
                receivable_amount,
                operation_total_count,
                remark,
            });
        }

        Ok(lines)
    }

    async fn prepare_payment(
        &self,
        request: SalesPaymentInput,
        receivable_amount: Decimal,
    ) -> Result<PaymentDraft, SalesRecordError> {
        let paid_amount = parse_positive_money("paid_amount", request.paid_amount)?;
        if paid_amount > receivable_amount {
            return Err(SalesRecordError::PaymentExceedsOutstanding);
        }
        let allocations = self
            .prepare_allocations(paid_amount, request.allocations)
            .await?;
        let remark = nullable_limited_text("remark", request.remark, MAX_REMARK_LENGTH)?;

        Ok(PaymentDraft {
            paid_amount,
            paid_at: request.paid_at,
            allocations,
            remark,
        })
    }

    async fn prepare_allocations(
        &self,
        paid_amount: Decimal,
        allocations: Vec<SalesPaymentAllocationInput>,
    ) -> Result<Vec<AllocationDraft>, SalesRecordError> {
        if allocations.is_empty() {
            return Err(SalesRecordError::PaymentAllocationsRequired);
        }

        let mut seen = std::collections::HashSet::new();
        let mut ratio_total = Decimal::ZERO;
        let mut drafts = Vec::with_capacity(allocations.len());
        for allocation in allocations {
            if !seen.insert(allocation.guide_user_id) {
                return Err(SalesRecordError::DuplicatePaymentGuide);
            }
            self.ensure_active_user("guide_user_id", allocation.guide_user_id)
                .await?;
            let ratio = parse_ratio("allocation_ratio", allocation.allocation_ratio)?;
            ratio_total += ratio;
            drafts.push(AllocationDraft {
                guide_user_id: allocation.guide_user_id,
                allocation_ratio: ratio,
                allocated_amount: Decimal::ZERO,
            });
        }

        if ratio_total != Decimal::new(10000, 2) {
            return Err(SalesRecordError::PaymentAllocationRatioTotalInvalid);
        }

        let mut allocated_total = Decimal::ZERO;
        let last_index = drafts.len().saturating_sub(1);
        for (index, draft) in drafts.iter_mut().enumerate() {
            if index == last_index {
                draft.allocated_amount = paid_amount - allocated_total;
            } else {
                let amount =
                    (paid_amount * draft.allocation_ratio / Decimal::new(100, 0)).round_dp(2);
                draft.allocated_amount = amount;
                allocated_total += amount;
            }
        }

        Ok(drafts)
    }

    async fn insert_allocations(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        payment_id: Uuid,
        paid_amount: Decimal,
        allocations: Vec<AllocationDraft>,
        now: DateTime<Utc>,
    ) -> Result<(), SalesRecordError> {
        let mut allocated_total = Decimal::ZERO;
        for allocation in allocations {
            allocated_total += allocation.allocated_amount;
            self.sales_records
                .insert_sales_payment_allocation(
                    tx,
                    NewSalesPaymentAllocation {
                        payment_id,
                        guide_user_id: allocation.guide_user_id,
                        allocation_ratio: allocation.allocation_ratio,
                        allocated_amount: allocation.allocated_amount,
                    },
                    now,
                )
                .await?;
        }
        if allocated_total != paid_amount {
            warn!(
                %payment_id,
                paid_amount = %format_money(paid_amount),
                allocated_total = %format_money(allocated_total),
                "payment allocation amount total did not match paid amount"
            );
            return Err(SalesRecordError::PaymentAllocationAmountTotalInvalid);
        }
        Ok(())
    }

    async fn customer_scope(&self, customer_id: Uuid) -> Result<CustomerScope, SalesRecordError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(SalesRecordError::CustomerNotFound)?;
        if customer.status != "active" {
            return Err(SalesRecordError::CustomerDisabled);
        }
        let system = self
            .systems
            .find_by_id(customer.system_id)
            .await?
            .ok_or(SalesRecordError::SystemNotFound)?;
        if system.status != "active" {
            return Err(SalesRecordError::SystemDisabled);
        }
        let store = self
            .stores
            .find_by_id(customer.store_id)
            .await?
            .ok_or(SalesRecordError::StoreNotFound)?;
        if store.status != "active" {
            return Err(SalesRecordError::StoreDisabled);
        }
        if store.system_id != customer.system_id {
            return Err(SalesRecordError::StoreSystemMismatch);
        }
        Ok(CustomerScope {
            system_id: customer.system_id,
            store_id: customer.store_id,
        })
    }

    async fn ensure_record_users(&self, users: RecordUserInput) -> Result<(), SalesRecordError> {
        self.ensure_active_user("handler_user_id", users.handler_user_id)
            .await?;
        self.ensure_optional_active_user("expert_user_id", users.expert_user_id)
            .await?;
        self.ensure_optional_active_user("consultant_user_id", users.consultant_user_id)
            .await?;
        self.ensure_optional_active_user("doctor_user_id", users.doctor_user_id)
            .await
    }

    async fn ensure_active_user(
        &self,
        field: &'static str,
        user_id: Uuid,
    ) -> Result<(), SalesRecordError> {
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .ok_or(SalesRecordError::UserNotFound { field })?;
        if user.status != "active" {
            return Err(SalesRecordError::ReferencedUserDisabled { field });
        }
        Ok(())
    }

    async fn ensure_optional_active_user(
        &self,
        field: &'static str,
        user_id: Option<Uuid>,
    ) -> Result<(), SalesRecordError> {
        if let Some(user_id) = user_id {
            self.ensure_active_user(field, user_id).await?;
        }
        Ok(())
    }

    async fn ensure_active_product(
        &self,
        product_id: Uuid,
    ) -> Result<product_entity::Model, SalesRecordError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(SalesRecordError::ProductNotFound)?;
        if product.status != "active" {
            return Err(SalesRecordError::ProductDisabled);
        }
        Ok(product)
    }

    async fn ensure_active_category(
        &self,
        category_id: Uuid,
    ) -> Result<product_category::Model, SalesRecordError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(SalesRecordError::ProductCategoryNotFound)?;
        if category.status != "active" {
            return Err(SalesRecordError::ProductCategoryDisabled);
        }
        Ok(category)
    }

    fn validate_sale_operation_count(
        &self,
        requires_operation_count: bool,
        total_count: Option<i32>,
    ) -> Result<Option<i32>, SalesRecordError> {
        match (requires_operation_count, total_count) {
            (true, Some(total_count)) => {
                validate_positive_count("operation_total_count", total_count)?;
                Ok(Some(total_count))
            }
            (true, None) => Err(SalesRecordError::OperationTotalCountRequired),
            (false, Some(_)) => Err(SalesRecordError::OperationTotalCountNotAllowed),
            (false, None) => Ok(None),
        }
    }

    async fn sales_record_remaining_amount_in<C: sea_orm::ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_id: Uuid,
    ) -> Result<Decimal, SalesRecordError> {
        let lines = self
            .sales_records
            .find_lines_by_sales_record_id_in(conn, sales_record_id)
            .await?;
        let payments = self
            .sales_records
            .find_payments_by_sales_record_id_in(conn, sales_record_id)
            .await?;
        let receivable = lines
            .into_iter()
            .filter(|line| line.status == "active")
            .map(|line| line.receivable_amount)
            .sum::<Decimal>();
        let paid = payments
            .into_iter()
            .filter(|payment| payment.status == "active")
            .map(|payment| payment.paid_amount)
            .sum::<Decimal>();
        Ok(receivable - paid)
    }

    async fn sales_record_response(
        &self,
        record: sales_records::Model,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let mut records = self.sales_record_responses(vec![record]).await?;
        Ok(records.remove(0))
    }

    async fn sales_record_responses(
        &self,
        records: Vec<sales_records::Model>,
    ) -> Result<Vec<SalesRecordResponse>, SalesRecordError> {
        let record_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        let lines = self
            .sales_records
            .find_lines_by_sales_record_ids(record_ids.clone())
            .await?;
        let line_ids = lines.iter().map(|line| line.id).collect::<Vec<_>>();
        let counts = self
            .sales_records
            .find_operation_counts_by_line_ids(line_ids)
            .await?;
        let payments = self
            .sales_records
            .find_payments_by_sales_record_ids(record_ids)
            .await?;
        let payment_ids = payments
            .iter()
            .map(|payment| payment.id)
            .collect::<Vec<_>>();
        let allocations = self
            .sales_records
            .find_allocations_by_payment_ids(payment_ids)
            .await?;

        let counts_by_line_id: HashMap<Uuid, sales_record_operation_counts::Model> = counts
            .into_iter()
            .map(|count| (count.sales_record_line_id, count))
            .collect();
        let mut lines_by_record_id: HashMap<Uuid, Vec<SalesRecordLineResponse>> = HashMap::new();
        for line in lines {
            let count = counts_by_line_id.get(&line.id).cloned();
            lines_by_record_id
                .entry(line.sales_record_id)
                .or_default()
                .push(SalesRecordLineResponse::from_model(line, count));
        }

        let mut allocations_by_payment_id: HashMap<Uuid, Vec<SalesPaymentAllocationResponse>> =
            HashMap::new();
        for allocation in allocations {
            allocations_by_payment_id
                .entry(allocation.payment_id)
                .or_default()
                .push(SalesPaymentAllocationResponse::from(allocation));
        }
        let mut payments_by_record_id: HashMap<Uuid, Vec<SalesPaymentResponse>> = HashMap::new();
        for payment in payments {
            let allocations = allocations_by_payment_id
                .remove(&payment.id)
                .unwrap_or_default();
            payments_by_record_id
                .entry(payment.sales_record_id)
                .or_default()
                .push(SalesPaymentResponse::from_parts(payment, allocations));
        }

        Ok(records
            .into_iter()
            .map(|record| {
                let lines = lines_by_record_id.remove(&record.id).unwrap_or_default();
                let payments = payments_by_record_id.remove(&record.id).unwrap_or_default();
                SalesRecordResponse::from_parts(record, lines, payments)
            })
            .collect())
    }

    async fn sales_payment_responses(
        &self,
        payments: Vec<sales_payments::Model>,
    ) -> Result<Vec<SalesPaymentResponse>, SalesRecordError> {
        let payment_ids = payments
            .iter()
            .map(|payment| payment.id)
            .collect::<Vec<_>>();
        let allocations = self
            .sales_records
            .find_allocations_by_payment_ids(payment_ids)
            .await?;
        let mut allocations_by_payment_id: HashMap<Uuid, Vec<SalesPaymentAllocationResponse>> =
            HashMap::new();
        for allocation in allocations {
            allocations_by_payment_id
                .entry(allocation.payment_id)
                .or_default()
                .push(SalesPaymentAllocationResponse::from(allocation));
        }

        Ok(payments
            .into_iter()
            .map(|payment| {
                let allocations = allocations_by_payment_id
                    .remove(&payment.id)
                    .unwrap_or_default();
                SalesPaymentResponse::from_parts(payment, allocations)
            })
            .collect())
    }

    async fn ensure_no_active_usages(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        sales_record_line_ids: Vec<Uuid>,
    ) -> Result<(), SalesRecordError> {
        let active_usage_count = self
            .sales_records
            .count_active_operation_usages_for_lines(tx, sales_record_line_ids)
            .await?;
        if active_usage_count > 0 {
            return Err(SalesRecordError::SalesRecordHasActiveUsages);
        }
        Ok(())
    }

    /// 明细行 → 所属销售记录 id(响应回显用)。
    async fn record_id_for_line(
        &self,
        sales_record_line_id: Uuid,
    ) -> Result<Uuid, SalesRecordError> {
        let line = self
            .sales_records
            .find_line_by_id(sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordLineNotFound)?;
        Ok(line.sales_record_id)
    }

    async fn record_ids_by_line_ids(
        &self,
        line_ids: impl Iterator<Item = Uuid>,
    ) -> Result<std::collections::HashMap<Uuid, Uuid>, SalesRecordError> {
        let unique: Vec<Uuid> = line_ids
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let lines = self.sales_records.find_lines_by_ids(unique).await?;
        Ok(lines
            .into_iter()
            .map(|line| (line.id, line.sales_record_id))
            .collect())
    }

    async fn ensure_active_line_record(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        line: &sales_record_lines::Model,
    ) -> Result<(), SalesRecordError> {
        if line.status == "voided" {
            return Err(SalesRecordError::SalesRecordLineVoided);
        }
        let record = self
            .sales_records
            .find_sales_record_by_id_for_update(tx, line.sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            return Err(SalesRecordError::SalesRecordVoided);
        }
        if record.record_type != "sale" {
            return Err(SalesRecordError::OperationCountRequiresSaleRecord);
        }
        Ok(())
    }

    async fn apply_usage_delta(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        count: &sales_record_operation_counts::Model,
        delta: i32,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, SalesRecordError> {
        if count.status == "voided" {
            return Err(SalesRecordError::OperationCountVoided);
        }
        let new_used_count = count.used_count + delta;
        if new_used_count < 0 {
            return Err(SalesRecordError::OperationCountBelowUsed);
        }
        if new_used_count > count.total_count {
            return Err(SalesRecordError::OperationCountInsufficient);
        }

        self.sales_records
            .update_operation_count(
                tx,
                count,
                OperationCountChanges {
                    used_count: Some(new_used_count),
                    ..OperationCountChanges::default()
                },
                now,
            )
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone, Copy)]
struct CustomerScope {
    system_id: Uuid,
    store_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
struct RecordUserInput {
    handler_user_id: Uuid,
    expert_user_id: Option<Uuid>,
    consultant_user_id: Option<Uuid>,
    doctor_user_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct LineDraft {
    product_id: Uuid,
    item_name: String,
    receivable_amount: Decimal,
    operation_total_count: Option<i32>,
    remark: Option<String>,
}

#[derive(Debug, Clone)]
struct PaymentDraft {
    paid_amount: Decimal,
    paid_at: DateTime<Utc>,
    allocations: Vec<AllocationDraft>,
    remark: Option<String>,
}

#[derive(Debug, Clone)]
struct AllocationDraft {
    guide_user_id: Uuid,
    allocation_ratio: Decimal,
    allocated_amount: Decimal,
}

#[derive(Debug, Error)]
pub enum SalesRecordError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("sales record was not found")]
    SalesRecordNotFound,
    #[error("sales record line was not found")]
    SalesRecordLineNotFound,
    #[error("sales payment was not found")]
    SalesPaymentNotFound,
    #[error("operation count was not found")]
    OperationCountNotFound,
    #[error("operation usage was not found")]
    OperationUsageNotFound,
    #[error("customer was not found")]
    CustomerNotFound,
    #[error("customer is disabled")]
    CustomerDisabled,
    #[error("system was not found")]
    SystemNotFound,
    #[error("system is disabled")]
    SystemDisabled,
    #[error("store was not found")]
    StoreNotFound,
    #[error("store is disabled")]
    StoreDisabled,
    #[error("product was not found")]
    ProductNotFound,
    #[error("product is disabled")]
    ProductDisabled,
    #[error("product category was not found")]
    ProductCategoryNotFound,
    #[error("product category is disabled")]
    ProductCategoryDisabled,
    #[error("{field} user was not found")]
    UserNotFound { field: &'static str },
    #[error("{field} user is disabled")]
    ReferencedUserDisabled { field: &'static str },
    #[error("store does not belong to the selected system")]
    StoreSystemMismatch,
    #[error("sales record must include at least one line")]
    SalesRecordLinesRequired,
    #[error("payment allocations must include at least one guide")]
    PaymentAllocationsRequired,
    #[error("guide_user_id cannot repeat in one payment")]
    DuplicatePaymentGuide,
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("{field} must be at most {maximum} characters")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("{field} must be one of: {expected}")]
    InvalidEnumValue {
        field: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("{field} must be a decimal string with at most two decimal places")]
    InvalidMoney { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to 0.00")]
    NegativeMoney { field: &'static str, value: String },
    #[error("{field} must be greater than 0.00")]
    NonPositiveMoney { field: &'static str, value: String },
    #[error("{field} must be less than or equal to 9999999999.99")]
    MoneyTooLarge { field: &'static str, value: String },
    #[error("{field} must be a decimal string with at most two decimal places")]
    InvalidRatio { field: &'static str, value: String },
    #[error("{field} must be greater than 0.00 and less than or equal to 100.00")]
    RatioOutOfRange { field: &'static str, value: String },
    #[error("payment allocation ratios must total 100.00")]
    PaymentAllocationRatioTotalInvalid,
    #[error("payment allocation amounts must total paid_amount")]
    PaymentAllocationAmountTotalInvalid,
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidCountMinimum { field: &'static str, minimum: i32 },
    #[error("operation_total_count is required for this product category")]
    OperationTotalCountRequired,
    #[error("operation_total_count is not allowed for this product category")]
    OperationTotalCountNotAllowed,
    #[error("service records cannot create operation counts")]
    ServiceOperationCountNotAllowed,
    #[error("sale records must have receivable amount greater than 0.00")]
    SaleReceivableRequired,
    #[error("service record line receivable_amount must be 0.00")]
    ServiceReceivableMustBeZero,
    #[error("payment amount exceeds outstanding amount")]
    PaymentExceedsOutstanding,
    #[error("collection payment must reference a sale record")]
    CollectionRequiresSaleRecord,
    #[error("operation counts can only be used on sale record lines")]
    OperationCountRequiresSaleRecord,
    #[error("sales record is voided")]
    SalesRecordVoided,
    #[error("sales record line is voided")]
    SalesRecordLineVoided,
    #[error("operation count is voided")]
    OperationCountVoided,
    #[error("operation usage is voided")]
    OperationUsageVoided,
    #[error("operation count is not enough")]
    OperationCountInsufficient,
    #[error("total_count cannot be less than used_count")]
    OperationCountBelowUsed,
    #[error("sales record has active operation usages")]
    SalesRecordHasActiveUsages,
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
    #[error("invalid record type")]
    InvalidRecordTypeInternal,
}

impl SalesRecordError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::SalesRecordNotFound => "sales_record_not_found",
            Self::SalesRecordLineNotFound => "sales_record_line_not_found",
            Self::SalesPaymentNotFound => "sales_payment_not_found",
            Self::OperationCountNotFound => "operation_count_not_found",
            Self::OperationUsageNotFound => "operation_usage_not_found",
            Self::CustomerNotFound => "customer_not_found",
            Self::SystemNotFound => "system_not_found",
            Self::StoreNotFound => "store_not_found",
            Self::ProductNotFound => "product_not_found",
            Self::ProductCategoryNotFound => "product_category_not_found",
            Self::UserNotFound { .. } => "user_not_found",
            Self::CustomerDisabled => "customer_disabled",
            Self::SystemDisabled => "system_disabled",
            Self::StoreDisabled => "store_disabled",
            Self::ProductDisabled => "product_disabled",
            Self::ProductCategoryDisabled => "product_category_disabled",
            Self::ReferencedUserDisabled { .. } => "referenced_user_disabled",
            Self::StoreSystemMismatch => "store_system_mismatch",
            Self::PaymentExceedsOutstanding => "payment_exceeds_outstanding",
            Self::CollectionRequiresSaleRecord => "collection_requires_sale_record",
            Self::SalesRecordVoided => "sales_record_voided",
            Self::SalesRecordLineVoided => "sales_record_line_voided",
            Self::OperationCountVoided => "operation_count_voided",
            Self::OperationUsageVoided => "operation_usage_voided",
            Self::OperationCountInsufficient => "operation_count_insufficient",
            Self::OperationCountBelowUsed => "operation_count_below_used",
            Self::SalesRecordHasActiveUsages => "sales_record_has_active_operation_usages",
            Self::OperationCountRequiresSaleRecord => "operation_count_requires_sale_record",
            Self::SalesRecordLinesRequired
            | Self::PaymentAllocationsRequired
            | Self::DuplicatePaymentGuide
            | Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidEnumValue { .. }
            | Self::InvalidMoney { .. }
            | Self::NegativeMoney { .. }
            | Self::NonPositiveMoney { .. }
            | Self::MoneyTooLarge { .. }
            | Self::InvalidRatio { .. }
            | Self::RatioOutOfRange { .. }
            | Self::PaymentAllocationRatioTotalInvalid
            | Self::PaymentAllocationAmountTotalInvalid
            | Self::InvalidCountMinimum { .. }
            | Self::OperationTotalCountRequired
            | Self::OperationTotalCountNotAllowed
            | Self::ServiceOperationCountNotAllowed
            | Self::SaleReceivableRequired
            | Self::ServiceReceivableMustBeZero
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. }
            | Self::InvalidRecordTypeInternal => "validation_error",
        }
    }
}

impl From<EnumParseError> for SalesRecordError {
    fn from(error: EnumParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            expected = error.expected,
            "rejected invalid sales record enum value"
        );
        Self::InvalidEnumValue {
            field: error.field,
            value: error.value,
            expected: error.expected,
        }
    }
}

fn active_receivable(lines: &[LineDraft]) -> Decimal {
    lines
        .iter()
        .map(|line| line.receivable_amount)
        .sum::<Decimal>()
}

fn optional_enum_text(
    field: &'static str,
    value: Option<String>,
    parser: fn(&'static str, &str) -> Result<&'static str, EnumParseError>,
) -> Result<Option<String>, SalesRecordError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    parser(field, value)
        .map(|value| Some(value.to_string()))
        .map_err(Into::into)
}

fn required_limited_text(
    field: &'static str,
    value: String,
    maximum: usize,
) -> Result<String, SalesRecordError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SalesRecordError::MissingRequiredField { field });
    }
    if value.chars().count() > maximum {
        return Err(SalesRecordError::FieldTooLong { field, maximum });
    }
    Ok(value.to_string())
}

fn nullable_limited_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, SalesRecordError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum {
        return Err(SalesRecordError::FieldTooLong { field, maximum });
    }
    Ok(Some(value.to_string()))
}

fn parse_money(field: &'static str, value: Option<String>) -> Result<Decimal, SalesRecordError> {
    let Some(value) = value else {
        return Err(SalesRecordError::MissingRequiredField { field });
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SalesRecordError::MissingRequiredField { field });
    }
    let decimal = Decimal::from_str(trimmed).map_err(|_| SalesRecordError::InvalidMoney {
        field,
        value: value.clone(),
    })?;
    if decimal.scale() > 2 {
        return Err(SalesRecordError::InvalidMoney { field, value });
    }
    if decimal < Decimal::ZERO {
        return Err(SalesRecordError::NegativeMoney { field, value });
    }
    if decimal > max_money() {
        return Err(SalesRecordError::MoneyTooLarge { field, value });
    }
    let mut normalized = decimal;
    normalized.rescale(2);
    Ok(normalized)
}

fn parse_positive_money(field: &'static str, value: String) -> Result<Decimal, SalesRecordError> {
    let parsed = parse_money(field, Some(value.clone()))?;
    if parsed <= Decimal::ZERO {
        return Err(SalesRecordError::NonPositiveMoney { field, value });
    }
    Ok(parsed)
}

fn parse_ratio(field: &'static str, value: String) -> Result<Decimal, SalesRecordError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SalesRecordError::MissingRequiredField { field });
    }
    let decimal = Decimal::from_str(trimmed).map_err(|_| SalesRecordError::InvalidRatio {
        field,
        value: value.clone(),
    })?;
    if decimal.scale() > 2 {
        return Err(SalesRecordError::InvalidRatio { field, value });
    }
    if decimal <= Decimal::ZERO || decimal > Decimal::new(10000, 2) {
        return Err(SalesRecordError::RatioOutOfRange { field, value });
    }
    let mut normalized = decimal;
    normalized.rescale(2);
    Ok(normalized)
}

fn max_money() -> Decimal {
    Decimal::new(999_999_999_999, 2)
}

fn validate_positive_count(field: &'static str, value: i32) -> Result<(), SalesRecordError> {
    if value < 1 {
        return Err(SalesRecordError::InvalidCountMinimum { field, minimum: 1 });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), SalesRecordError> {
    if page_number == 0 {
        return Err(SalesRecordError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }
    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), SalesRecordError> {
    if page_size == 0 {
        return Err(SalesRecordError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }
    if page_size > MAX_PAGE_SIZE {
        return Err(SalesRecordError::InvalidPaginationMaximum {
            field: "page_size",
            maximum: MAX_PAGE_SIZE,
        });
    }
    Ok(())
}

fn operation_usage_changes(
    request: UpdateOperationUsageRequest,
) -> Result<OperationUsageChanges, SalesRecordError> {
    Ok(OperationUsageChanges {
        operated_at: required_datetime_change("operated_at", request.operated_at)?,
        operator_user_id: required_uuid_change("operator_user_id", request.operator_user_id)?,
        doctor_user_id: nullable_uuid_change(request.doctor_user_id),
        operation_count: count_change("operation_count", request.operation_count)?,
        remark: nullable_text_change("remark", request.remark, MAX_REMARK_LENGTH)?,
        status: None,
    })
}

fn required_uuid_change(
    field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Uuid>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn nullable_uuid_change(value: PatchField<Uuid>) -> Option<Option<Uuid>> {
    match value {
        PatchField::Unset => None,
        PatchField::Null => Some(None),
        PatchField::Value(value) => Some(Some(value)),
    }
}

fn required_datetime_change(
    field: &'static str,
    value: PatchField<DateTime<Utc>>,
) -> Result<Option<DateTime<Utc>>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn count_change(
    field: &'static str,
    value: PatchField<i32>,
) -> Result<Option<i32>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => {
            validate_positive_count(field, value)?;
            Ok(Some(value))
        }
    }
}

fn nullable_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<Option<String>>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => nullable_limited_text(field, Some(value), maximum).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        dto::sales_records::{
            CreateCollectionPaymentRequest, CreateOperationUsageRequest, CreateSaleRecordRequest,
            CreateServiceRecordRequest, SalesPaymentAllocationInput, SalesPaymentInput,
            SalesRecordLineInput,
        },
        repositories::{
            customers::{CustomerRepository, NewCustomer},
            product_categories::ProductCategoryRepository,
            products::{NewProduct, ProductRepository},
            sales_records::SalesRecordRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
    };
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    struct Harness {
        users: UserRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        products: ProductRepository,
        categories: ProductCategoryRepository,
        service: SalesRecordService,
    }

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    impl Harness {
        async fn new() -> Self {
            let db = db::connect_and_migrate(&sqlite_memory_config())
                .await
                .expect("sqlite memory database should initialize");
            let users = UserRepository::new(db.clone());
            let systems = SystemRepository::new(db.clone());
            let stores = StoreRepository::new(db.clone());
            let customers = CustomerRepository::new(db.clone());
            let products = ProductRepository::new(db.clone());
            let categories = ProductCategoryRepository::new(db.clone());
            let sales_records = SalesRecordRepository::new(db);
            let service = SalesRecordService::new(
                sales_records,
                customers.clone(),
                systems.clone(),
                stores.clone(),
                categories.clone(),
                users.clone(),
            );
            Self {
                users,
                systems,
                stores,
                customers,
                products,
                categories,
                service,
            }
        }

        async fn user(&self, name: &str) -> Uuid {
            self.users
                .find_or_create_for_login(name, Utc::now())
                .await
                .expect("user should be created")
                .id
        }

        async fn scope(&self, name: &str) -> (Uuid, Uuid) {
            let now = Utc::now();
            let system = self
                .systems
                .create_system(
                    NewSystem {
                        name: name.to_string(),
                        status: "active".to_string(),
                    },
                    now,
                )
                .await
                .expect("system should be created");
            let store = self
                .stores
                .create_store(
                    NewStore {
                        name: name.to_string(),
                        system_id: system.id,
                        status: "active".to_string(),
                    },
                    now,
                )
                .await
                .expect("store should be created");
            (system.id, store.id)
        }

        async fn customer(&self, creator_user_id: Uuid, system_id: Uuid, store_id: Uuid) -> Uuid {
            self.customers
                .create_customer(
                    NewCustomer {
                        name: "Alice".to_string(),
                        creator_user_id,
                        system_id,
                        store_id,
                        remark: None,
                        status: "active".to_string(),
                        attachments: None,
                    },
                    Utc::now(),
                )
                .await
                .expect("customer should be created")
                .id
        }

        async fn category(&self, requires_operation_count: bool) -> Uuid {
            self.categories
                .list_categories(Some("active"), Some(requires_operation_count), 1, 50)
                .await
                .expect("categories should list")
                .0
                .into_iter()
                .next()
                .expect("seed category should exist")
                .id
        }

        async fn product(&self, name: &str, requires_operation_count: bool) -> Uuid {
            let category_id = self.category(requires_operation_count).await;
            self.products
                .create_product(
                    NewProduct {
                        name: name.to_string(),
                        category_id,
                        series: None,
                        brand_name: None,
                        specification: None,
                        unit: Some("unit".to_string()),
                        unit_price: Decimal::new(10000, 2),
                        status: "active".to_string(),
                    },
                    Utc::now(),
                )
                .await
                .expect("product should be created")
                .id
        }
    }

    fn allocation(guide_user_id: Uuid, ratio: &str) -> SalesPaymentAllocationInput {
        SalesPaymentAllocationInput {
            guide_user_id,
            allocation_ratio: ratio.to_string(),
        }
    }

    fn sale_line(product_id: Uuid, amount: &str, count: Option<i32>) -> SalesRecordLineInput {
        SalesRecordLineInput {
            product_id,
            item_name: "item".to_string(),
            receivable_amount: amount.to_string(),
            operation_total_count: count,
            remark: None,
        }
    }

    fn payment(guide_user_id: Uuid, amount: &str) -> SalesPaymentInput {
        SalesPaymentInput {
            paid_amount: amount.to_string(),
            paid_at: Utc.with_ymd_and_hms(2026, 7, 8, 10, 0, 0).unwrap(),
            allocations: vec![allocation(guide_user_id, "100.00")],
            remark: None,
        }
    }

    async fn sale_request(
        h: &Harness,
        amount: &str,
        paid: &str,
    ) -> (Uuid, CreateSaleRecordRequest) {
        let actor = h.user("actor").await;
        let guide = h.user("guide").await;
        let (system_id, store_id) = h.scope("scope-a").await;
        let customer_id = h.customer(actor, system_id, store_id).await;
        let product_id = h.product("operation item", true).await;
        (
            actor,
            CreateSaleRecordRequest {
                customer_id,
                record_date: Utc
                    .with_ymd_and_hms(2026, 7, 8, 0, 0, 0)
                    .unwrap()
                    .date_naive(),
                customer_type: "new".to_string(),
                deal_type: "non_salon".to_string(),
                handler_user_id: actor,
                expert_user_id: None,
                consultant_user_id: None,
                doctor_user_id: None,
                remark: Some(" sale ".to_string()),
                lines: vec![sale_line(product_id, amount, Some(3))],
                payment: payment(guide, paid),
            },
        )
    }

    #[tokio::test]
    async fn creates_sale_record_with_initial_payment_and_operation_count() {
        let h = Harness::new().await;
        let (actor, request) = sale_request(&h, "300.00", "100.00").await;

        let created = h
            .service
            .create_sale_record(actor, request)
            .await
            .expect("sale record should be created");

        assert_eq!(created.record_type, "sale");
        assert_eq!(created.receivable_amount, "300.00");
        assert_eq!(created.paid_amount, "100.00");
        assert_eq!(created.outstanding_amount, "200.00");
        assert_eq!(created.lines.len(), 1);
        assert_eq!(
            created.lines[0]
                .operation_count
                .as_ref()
                .map(|count| count.total_count),
            Some(3)
        );
        assert_eq!(created.payments.len(), 1);
        assert_eq!(created.payments[0].payment_type, "initial");
        assert_eq!(
            created.payments[0].allocations[0].allocated_amount,
            "100.00"
        );
    }

    #[tokio::test]
    async fn creates_service_record_without_payment_or_counts() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let (system_id, store_id) = h.scope("scope-a").await;
        let customer_id = h.customer(actor, system_id, store_id).await;
        let product_id = h.product("service content", true).await;

        let created = h
            .service
            .create_service_record(
                actor,
                CreateServiceRecordRequest {
                    customer_id,
                    record_date: Utc
                        .with_ymd_and_hms(2026, 7, 8, 0, 0, 0)
                        .unwrap()
                        .date_naive(),
                    customer_type: None,
                    deal_type: None,
                    handler_user_id: actor,
                    expert_user_id: None,
                    consultant_user_id: None,
                    doctor_user_id: None,
                    remark: None,
                    lines: vec![sale_line(product_id, "0.00", None)],
                },
            )
            .await
            .expect("service record should be created");

        assert_eq!(created.record_type, "service");
        assert_eq!(created.receivable_amount, "0.00");
        assert_eq!(created.paid_amount, "0.00");
        assert_eq!(created.outstanding_amount, "0.00");
        assert!(created.lines[0].operation_count.is_none());
        assert!(created.payments.is_empty());

        assert!(matches!(
            h.service
                .create_service_record(
                    actor,
                    CreateServiceRecordRequest {
                        lines: vec![sale_line(product_id, "1.00", None)],
                        ..CreateServiceRecordRequest {
                            customer_id,
                            record_date: Utc
                                .with_ymd_and_hms(2026, 7, 8, 0, 0, 0)
                                .unwrap()
                                .date_naive(),
                            customer_type: None,
                            deal_type: None,
                            handler_user_id: actor,
                            expert_user_id: None,
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            lines: Vec::new(),
                        }
                    },
                )
                .await,
            Err(SalesRecordError::ServiceReceivableMustBeZero)
        ));
    }

    #[tokio::test]
    async fn collection_payment_updates_outstanding_amount() {
        let h = Harness::new().await;
        let (actor, request) = sale_request(&h, "300.00", "100.00").await;
        let created = h
            .service
            .create_sale_record(actor, request)
            .await
            .expect("sale record should be created");
        let guide = h.user("second-guide").await;

        let over_collection = h
            .service
            .create_collection_payment(
                actor,
                CreateCollectionPaymentRequest {
                    sales_record_id: created.id,
                    paid_amount: "201.00".to_string(),
                    paid_at: Utc.with_ymd_and_hms(2026, 7, 9, 10, 0, 0).unwrap(),
                    allocations: vec![allocation(guide, "100.00")],
                    remark: None,
                },
            )
            .await;
        assert!(
            matches!(
                over_collection,
                Err(SalesRecordError::PaymentExceedsOutstanding)
            ),
            "{over_collection:?}"
        );

        let payment = h
            .service
            .create_collection_payment(
                actor,
                CreateCollectionPaymentRequest {
                    sales_record_id: created.id,
                    paid_amount: "200.00".to_string(),
                    paid_at: Utc.with_ymd_and_hms(2026, 7, 9, 10, 0, 0).unwrap(),
                    allocations: vec![allocation(guide, "100.00")],
                    remark: Some("collection".to_string()),
                },
            )
            .await
            .expect("collection should be created");
        assert_eq!(payment.payment_type, "collection");
        assert_eq!(payment.paid_amount, "200.00");

        let detail = h
            .service
            .sales_record_detail(created.id)
            .await
            .expect("record should load");
        assert_eq!(detail.paid_amount, "300.00");
        assert_eq!(detail.outstanding_amount, "0.00");
    }

    #[tokio::test]
    async fn operation_usage_lifecycle_uses_sales_record_line_id() {
        let h = Harness::new().await;
        let (actor, request) = sale_request(&h, "300.00", "100.00").await;
        let created = h
            .service
            .create_sale_record(actor, request)
            .await
            .expect("sale record should be created");
        let line_id = created.lines[0].id;
        let operator = h.user("operator").await;

        let usage = h
            .service
            .create_operation_usage(CreateOperationUsageRequest {
                sales_record_line_id: line_id,
                operated_at: Utc.with_ymd_and_hms(2026, 7, 9, 9, 0, 0).unwrap(),
                operator_user_id: operator,
                doctor_user_id: None,
                operation_count: 2,
                remark: None,
            })
            .await
            .expect("usage should be created");
        assert_eq!(usage.sales_record_id, created.id);
        let count_detail = h
            .service
            .operation_count_detail(line_id)
            .await
            .expect("count should load");
        assert_eq!(count_detail.used_count, 2);
        assert_eq!(count_detail.sales_record_id, created.id);

        let voided = h
            .service
            .void_operation_usage(usage.id)
            .await
            .expect("usage should void");
        assert_eq!(voided.sales_record_id, created.id);
        assert_eq!(
            h.service
                .operation_count_detail(line_id)
                .await
                .expect("count should load")
                .used_count,
            0
        );
    }

    #[tokio::test]
    async fn lists_counts_and_usages_filtered_by_sales_record_id() {
        let h = Harness::new().await;
        let (actor, request) = sale_request(&h, "300.00", "100.00").await;
        let first = h
            .service
            .create_sale_record(actor, request.clone())
            .await
            .expect("first sale record should be created");
        let second = h
            .service
            .create_sale_record(actor, request)
            .await
            .expect("second sale record should be created");
        let operator = h.user("operator").await;
        for record in [&first, &second] {
            h.service
                .create_operation_usage(CreateOperationUsageRequest {
                    sales_record_line_id: record.lines[0].id,
                    operated_at: Utc.with_ymd_and_hms(2026, 7, 9, 9, 0, 0).unwrap(),
                    operator_user_id: operator,
                    doctor_user_id: None,
                    operation_count: 1,
                    remark: None,
                })
                .await
                .expect("usage should be created");
        }

        let counts = h
            .service
            .list_operation_counts(ListOperationCountsQuery {
                sales_record_id: Some(first.id),
                ..ListOperationCountsQuery::default()
            })
            .await
            .expect("counts should list");
        assert_eq!(counts.total_count, 1);
        assert_eq!(counts.operation_counts[0].sales_record_id, first.id);
        assert_eq!(
            counts.operation_counts[0].sales_record_line_id,
            first.lines[0].id
        );

        let usages = h
            .service
            .list_operation_usages(ListOperationUsagesQuery {
                sales_record_id: Some(second.id),
                ..ListOperationUsagesQuery::default()
            })
            .await
            .expect("usages should list");
        assert_eq!(usages.total_count, 1);
        assert_eq!(usages.operation_usages[0].sales_record_id, second.id);
        assert_eq!(
            usages.operation_usages[0].sales_record_line_id,
            second.lines[0].id
        );
    }
}
