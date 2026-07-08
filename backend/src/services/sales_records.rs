use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::sales_records::{
        CreateOperationUsageRequest, CreateSalesRecordBatchRequest, CreateSalesRecordBatchResponse,
        EnumParseError, ListOperationCountsQuery, ListOperationCountsResponse,
        ListOperationUsagesQuery, ListOperationUsagesResponse, ListSalesRecordsQuery,
        ListSalesRecordsResponse, OperationCountResponse, OperationUsageResponse, PatchField,
        SalesRecordResponse, UpdateOperationCountRequest, UpdateOperationUsageRequest,
        UpdateSalesRecordRequest, parse_collaboration_type, parse_customer_type, parse_deal_status,
        parse_deal_type, parse_status,
    },
    entities::{product_category, sales_record_operation_counts, sales_records},
    repositories::{
        RepositoryError,
        customers::CustomerRepository,
        departments::DepartmentRepository,
        product_categories::ProductCategoryRepository,
        sales_records::{
            NewOperationCount, NewOperationUsage, NewSalesRecord, OperationCountChanges,
            OperationCountFilters, OperationUsageChanges, OperationUsageFilters,
            SalesRecordChanges, SalesRecordFilters, SalesRecordRepository,
        },
        stores::StoreRepository,
        systems::SystemRepository,
        users::UserRepository,
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_REMARK_LENGTH: usize = 2000;

#[derive(Clone)]
pub struct SalesRecordService {
    sales_records: SalesRecordRepository,
    customers: CustomerRepository,
    departments: DepartmentRepository,
    systems: SystemRepository,
    stores: StoreRepository,
    categories: ProductCategoryRepository,
    users: UserRepository,
}

impl SalesRecordService {
    pub fn new(
        sales_records: SalesRecordRepository,
        customers: CustomerRepository,
        departments: DepartmentRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        categories: ProductCategoryRepository,
        users: UserRepository,
    ) -> Self {
        Self {
            sales_records,
            customers,
            departments,
            systems,
            stores,
            categories,
            users,
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_sales_record_batch(
        &self,
        request: CreateSalesRecordBatchRequest,
    ) -> Result<CreateSalesRecordBatchResponse, SalesRecordError> {
        if request.records.is_empty() {
            return Err(SalesRecordError::EmptyBatch);
        }

        let record_group_id = Uuid::new_v4();
        let mut normalized = Vec::with_capacity(request.records.len());
        for item in request.records {
            normalized.push(self.new_sales_record(record_group_id, item).await?);
        }

        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let mut created = Vec::with_capacity(normalized.len());

        for (record, operation_total_count) in normalized {
            let record = self
                .sales_records
                .insert_sales_record(&tx, record, now)
                .await?;
            let count = match operation_total_count {
                Some(total_count) => Some(
                    self.sales_records
                        .insert_operation_count(
                            &tx,
                            NewOperationCount {
                                sales_record_id: record.id,
                                total_count,
                                used_count: 0,
                                status: "active".to_string(),
                            },
                            now,
                        )
                        .await?,
                ),
                None => None,
            };
            created.push(SalesRecordResponse::from_model(record, count));
        }

        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%record_group_id, count = created.len(), "created sales record batch");
        Ok(CreateSalesRecordBatchResponse {
            record_group_id,
            sales_records: created,
        })
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
        let (records, total_count) = self
            .sales_records
            .list_sales_records(
                SalesRecordFilters {
                    status_filter,
                    record_group_id: query.record_group_id,
                    customer_id: query.customer_id,
                    department_id: query.department_id,
                    system_id: query.system_id,
                    store_id: query.store_id,
                    handler_user_id: query.handler_user_id,
                    content_category_id: query.content_category_id,
                    sale_date_from: query.sale_date_from,
                    sale_date_to: query.sale_date_to,
                },
                page_number,
                page_size,
            )
            .await?;
        let records = self.sales_record_responses(records).await?;

        debug!(
            count = records.len(),
            total_count, page_number, page_size, "listed sales records through service"
        );
        Ok(ListSalesRecordsResponse {
            sales_records: records,
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

        debug!(%sales_record_id, "loaded sales record detail");
        self.sales_record_response(record).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_sales_record(
        &self,
        sales_record_id: Uuid,
        request: UpdateSalesRecordRequest,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let record = self
            .sales_records
            .find_sales_record_by_id(sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            return Err(SalesRecordError::SalesRecordVoided);
        }

        let changes = self.sales_record_changes(&record, request).await?;
        if changes.is_empty() {
            debug!(%sales_record_id, "sales record update request had no changes");
            return self.sales_record_response(record).await;
        }

        let record = self
            .sales_records
            .update_sales_record(&self.sales_records.db, &record, changes, Utc::now())
            .await?;
        info!(%sales_record_id, "updated sales record through service");
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
            return self.sales_record_response(record).await;
        }
        self.ensure_no_active_usages(&tx, sales_record_id).await?;

        let record = self
            .sales_records
            .update_sales_record_status(&tx, &record, "voided", now)
            .await?;
        if let Some(count) = self
            .sales_records
            .find_operation_count_for_update(&tx, sales_record_id)
            .await?
        {
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

        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%sales_record_id, "voided sales record through service");
        self.sales_record_response(record).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_sales_record(&self, sales_record_id: Uuid) -> Result<(), SalesRecordError> {
        let tx = self.sales_records.begin().await?;
        self.sales_records
            .find_sales_record_by_id_for_update(&tx, sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        self.ensure_no_active_usages(&tx, sales_record_id).await?;

        let deleted = self
            .sales_records
            .delete_sales_record_by_id(&tx, sales_record_id)
            .await?;
        if !deleted {
            return Err(SalesRecordError::SalesRecordNotFound);
        }

        tx.commit().await.map_err(RepositoryError::from)?;
        info!(%sales_record_id, "deleted sales record through service");
        Ok(())
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
                    sales_record_id: query.sales_record_id,
                },
                page_number,
                page_size,
            )
            .await?;

        Ok(ListOperationCountsResponse {
            operation_counts: counts
                .into_iter()
                .map(OperationCountResponse::from)
                .collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn operation_count_detail(
        &self,
        sales_record_id: Uuid,
    ) -> Result<OperationCountResponse, SalesRecordError> {
        let count = self
            .sales_records
            .find_operation_count(sales_record_id)
            .await?
            .ok_or(SalesRecordError::OperationCountNotFound)?;

        Ok(OperationCountResponse::from(count))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_operation_count(
        &self,
        sales_record_id: Uuid,
        request: UpdateOperationCountRequest,
    ) -> Result<OperationCountResponse, SalesRecordError> {
        validate_positive_count("total_count", request.total_count)?;
        let now = Utc::now();
        let tx = self.sales_records.begin().await?;
        let record = self
            .sales_records
            .find_sales_record_by_id_for_update(&tx, sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            return Err(SalesRecordError::SalesRecordVoided);
        }
        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, sales_record_id)
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
        info!(%sales_record_id, "updated operation count through service");
        Ok(OperationCountResponse::from(count))
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

        Ok(ListOperationUsagesResponse {
            operation_usages: usages
                .into_iter()
                .map(OperationUsageResponse::from)
                .collect(),
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

        Ok(OperationUsageResponse::from(usage))
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
        self.ensure_active_sales_record_for_usage(&tx, request.sales_record_id)
            .await?;
        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, request.sales_record_id)
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
                    sales_record_id: request.sales_record_id,
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
            sales_record_id = %usage.sales_record_id,
            used_count = count.used_count,
            "created operation usage through service"
        );
        Ok(OperationUsageResponse::from(usage))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_operation_usage(
        &self,
        usage_id: Uuid,
        request: UpdateOperationUsageRequest,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        let changes = operation_usage_changes(request).await?;
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
            return Ok(OperationUsageResponse::from(usage));
        }

        self.ensure_active_sales_record_for_usage(&tx, usage.sales_record_id)
            .await?;
        if let Some(new_count) = changes.operation_count {
            let delta = new_count - usage.operation_count;
            if delta != 0 {
                let count = self
                    .sales_records
                    .find_operation_count_for_update(&tx, usage.sales_record_id)
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
        info!(%usage_id, "updated operation usage through service");
        Ok(OperationUsageResponse::from(usage))
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
            return Ok(OperationUsageResponse::from(usage));
        }

        let count = self
            .sales_records
            .find_operation_count_for_update(&tx, usage.sales_record_id)
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
        info!(%usage_id, "voided operation usage through service");
        Ok(OperationUsageResponse::from(usage))
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
                .find_operation_count_for_update(&tx, usage.sales_record_id)
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
        info!(%usage_id, "deleted operation usage through service");
        Ok(())
    }

    async fn new_sales_record(
        &self,
        record_group_id: Uuid,
        request: crate::dto::sales_records::CreateSalesRecordRequest,
    ) -> Result<(NewSalesRecord, Option<i32>), SalesRecordError> {
        let deal_status = parse_deal_status("deal_status", &request.deal_status)?.to_string();
        let customer_type =
            parse_customer_type("customer_type", &request.customer_type)?.to_string();
        let deal_type = parse_deal_type("deal_type", &request.deal_type)?.to_string();
        let collaboration_type =
            parse_collaboration_type("collaboration_type", &request.collaboration_type)?
                .to_string();
        let paid_amount = parse_money("paid_amount", Some(request.paid_amount))?;
        let unpaid_amount = parse_money("unpaid_amount", Some(request.unpaid_amount))?;
        let category = self
            .ensure_category(request.content_category_id, true)
            .await?;
        let operation_total_count = self.validate_operation_total_count(
            category.requires_operation_count,
            request.operation_total_count,
        )?;

        self.ensure_sales_record_references(
            SalesRecordReferenceInput {
                customer_id: request.customer_id,
                department_id: request.department_id,
                system_id: request.system_id,
                store_id: request.store_id,
                handler_user_id: request.handler_user_id,
                expert_user_id: request.expert_user_id,
                expert_department_id: request.expert_department_id,
                consultant_user_id: request.consultant_user_id,
                consultant_department_id: request.consultant_department_id,
                doctor_user_id: request.doctor_user_id,
            },
            &collaboration_type,
        )
        .await?;

        Ok((
            NewSalesRecord {
                record_group_id: Some(record_group_id),
                customer_id: request.customer_id,
                department_id: request.department_id,
                sale_date: request.sale_date,
                deal_status,
                customer_type,
                deal_type,
                content_category_id: request.content_category_id,
                handler_user_id: request.handler_user_id,
                paid_amount,
                unpaid_amount,
                system_id: request.system_id,
                store_id: request.store_id,
                collaboration_type,
                expert_user_id: request.expert_user_id,
                expert_department_id: request.expert_department_id,
                consultant_user_id: request.consultant_user_id,
                consultant_department_id: request.consultant_department_id,
                doctor_user_id: request.doctor_user_id,
                status: "active".to_string(),
            },
            operation_total_count,
        ))
    }

    async fn sales_record_changes(
        &self,
        record: &sales_records::Model,
        request: UpdateSalesRecordRequest,
    ) -> Result<SalesRecordChanges, SalesRecordError> {
        let customer_id = required_uuid_change("customer_id", request.customer_id)?;
        let department_id = required_uuid_change("department_id", request.department_id)?;
        let sale_date = required_date_change("sale_date", request.sale_date)?;
        let deal_status = enum_text_change("deal_status", request.deal_status, parse_deal_status)?;
        let customer_type =
            enum_text_change("customer_type", request.customer_type, parse_customer_type)?;
        let deal_type = enum_text_change("deal_type", request.deal_type, parse_deal_type)?;
        let content_category_id =
            required_uuid_change("content_category_id", request.content_category_id)?;
        let handler_user_id = required_uuid_change("handler_user_id", request.handler_user_id)?;
        let paid_amount = money_change("paid_amount", request.paid_amount)?;
        let unpaid_amount = money_change("unpaid_amount", request.unpaid_amount)?;
        let system_id = required_uuid_change("system_id", request.system_id)?;
        let store_id = required_uuid_change("store_id", request.store_id)?;
        let collaboration_type = enum_text_change(
            "collaboration_type",
            request.collaboration_type,
            parse_collaboration_type,
        )?;
        let expert_user_id = nullable_uuid_change("expert_user_id", request.expert_user_id)?;
        let expert_department_id =
            nullable_uuid_change("expert_department_id", request.expert_department_id)?;
        let consultant_user_id =
            nullable_uuid_change("consultant_user_id", request.consultant_user_id)?;
        let consultant_department_id =
            nullable_uuid_change("consultant_department_id", request.consultant_department_id)?;
        let doctor_user_id = nullable_uuid_change("doctor_user_id", request.doctor_user_id)?;

        let final_customer_id = customer_id.unwrap_or(record.customer_id);
        let final_department_id = department_id.unwrap_or(record.department_id);
        let final_system_id = system_id.unwrap_or(record.system_id);
        let final_store_id = store_id.unwrap_or(record.store_id);
        let final_handler_user_id = handler_user_id.unwrap_or(record.handler_user_id);
        let final_collaboration_type = collaboration_type
            .as_deref()
            .unwrap_or(&record.collaboration_type)
            .to_string();
        let final_expert_user_id = expert_user_id.unwrap_or(record.expert_user_id);
        let final_expert_department_id =
            expert_department_id.unwrap_or(record.expert_department_id);
        let final_consultant_user_id = consultant_user_id.unwrap_or(record.consultant_user_id);
        let final_consultant_department_id =
            consultant_department_id.unwrap_or(record.consultant_department_id);
        let final_doctor_user_id = doctor_user_id.unwrap_or(record.doctor_user_id);

        self.ensure_sales_record_references(
            SalesRecordReferenceInput {
                customer_id: final_customer_id,
                department_id: final_department_id,
                system_id: final_system_id,
                store_id: final_store_id,
                handler_user_id: final_handler_user_id,
                expert_user_id: final_expert_user_id,
                expert_department_id: final_expert_department_id,
                consultant_user_id: final_consultant_user_id,
                consultant_department_id: final_consultant_department_id,
                doctor_user_id: final_doctor_user_id,
            },
            &final_collaboration_type,
        )
        .await?;

        if let Some(content_category_id) = content_category_id {
            let category = self.ensure_category(content_category_id, true).await?;
            let has_count = self
                .sales_records
                .find_operation_count(record.id)
                .await?
                .is_some();
            if has_count != category.requires_operation_count {
                return Err(SalesRecordError::ContentCategoryOperationCountMismatch);
            }
        }

        Ok(SalesRecordChanges {
            customer_id,
            department_id,
            sale_date,
            deal_status,
            customer_type,
            deal_type,
            content_category_id,
            handler_user_id,
            paid_amount,
            unpaid_amount,
            system_id,
            store_id,
            collaboration_type,
            expert_user_id,
            expert_department_id,
            consultant_user_id,
            consultant_department_id,
            doctor_user_id,
        })
    }

    async fn ensure_sales_record_references(
        &self,
        refs: SalesRecordReferenceInput,
        collaboration_type: &str,
    ) -> Result<(), SalesRecordError> {
        let customer = self
            .customers
            .find_by_id(refs.customer_id)
            .await?
            .ok_or(SalesRecordError::CustomerNotFound)?;
        if customer.status != "active" {
            return Err(SalesRecordError::CustomerDisabled);
        }
        if self
            .departments
            .find_by_id(refs.department_id)
            .await?
            .is_none()
        {
            return Err(SalesRecordError::DepartmentNotFound);
        }
        let system = self
            .systems
            .find_by_id(refs.system_id)
            .await?
            .ok_or(SalesRecordError::SystemNotFound)?;
        if system.department_id != refs.department_id {
            warn!(
                department_id = %refs.department_id,
                system_id = %refs.system_id,
                system_department_id = %system.department_id,
                "rejected sales record because system does not belong to department"
            );
            return Err(SalesRecordError::SystemDepartmentMismatch);
        }
        let store = self
            .stores
            .find_by_id(refs.store_id)
            .await?
            .ok_or(SalesRecordError::StoreNotFound)?;
        if store.system_id != refs.system_id {
            warn!(
                store_id = %refs.store_id,
                system_id = %refs.system_id,
                store_system_id = %store.system_id,
                "rejected sales record because store does not belong to system"
            );
            return Err(SalesRecordError::StoreSystemMismatch);
        }

        self.ensure_active_user("handler_user_id", refs.handler_user_id)
            .await?;
        self.ensure_optional_active_user("expert_user_id", refs.expert_user_id)
            .await?;
        self.ensure_optional_active_user("consultant_user_id", refs.consultant_user_id)
            .await?;
        self.ensure_optional_active_user("doctor_user_id", refs.doctor_user_id)
            .await?;
        if let Some(expert_department_id) = refs.expert_department_id
            && self
                .departments
                .find_by_id(expert_department_id)
                .await?
                .is_none()
        {
            return Err(SalesRecordError::DepartmentNotFound);
        }
        if let Some(consultant_department_id) = refs.consultant_department_id
            && self
                .departments
                .find_by_id(consultant_department_id)
                .await?
                .is_none()
        {
            return Err(SalesRecordError::DepartmentNotFound);
        }

        match collaboration_type {
            "expert_consultation" => {
                if refs.expert_user_id.is_none() || refs.expert_department_id.is_none() {
                    return Err(SalesRecordError::MissingExpertFields);
                }
            }
            "self_sale" => {
                if refs.expert_user_id.is_some() || refs.expert_department_id.is_some() {
                    return Err(SalesRecordError::UnexpectedExpertFields);
                }
            }
            _ => {
                return Err(SalesRecordError::InvalidEnumValue {
                    field: "collaboration_type",
                    value: collaboration_type.to_string(),
                    expected: "expert_consultation, self_sale",
                });
            }
        }

        Ok(())
    }

    async fn ensure_category(
        &self,
        category_id: Uuid,
        require_active: bool,
    ) -> Result<product_category::Model, SalesRecordError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(SalesRecordError::ProductCategoryNotFound)?;
        if require_active && category.status != "active" {
            return Err(SalesRecordError::ProductCategoryDisabled);
        }
        Ok(category)
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

    fn validate_operation_total_count(
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

    async fn sales_record_response(
        &self,
        record: sales_records::Model,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let count = self.sales_records.find_operation_count(record.id).await?;
        Ok(SalesRecordResponse::from_model(record, count))
    }

    async fn sales_record_responses(
        &self,
        records: Vec<sales_records::Model>,
    ) -> Result<Vec<SalesRecordResponse>, SalesRecordError> {
        let record_ids = records.iter().map(|record| record.id).collect();
        let counts = self
            .sales_records
            .find_operation_counts_by_sales_record_ids(record_ids)
            .await?;
        let counts_by_record_id: HashMap<Uuid, sales_record_operation_counts::Model> = counts
            .into_iter()
            .map(|count| (count.sales_record_id, count))
            .collect();

        Ok(records
            .into_iter()
            .map(|record| {
                let count = counts_by_record_id.get(&record.id).cloned();
                SalesRecordResponse::from_model(record, count)
            })
            .collect())
    }

    async fn ensure_no_active_usages(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        sales_record_id: Uuid,
    ) -> Result<(), SalesRecordError> {
        let active_usage_count = self
            .sales_records
            .count_active_operation_usages(tx, sales_record_id)
            .await?;
        if active_usage_count > 0 {
            return Err(SalesRecordError::SalesRecordHasActiveUsages);
        }
        Ok(())
    }

    async fn ensure_active_sales_record_for_usage(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        sales_record_id: Uuid,
    ) -> Result<(), SalesRecordError> {
        let record = self
            .sales_records
            .find_sales_record_by_id_for_update(tx, sales_record_id)
            .await?
            .ok_or(SalesRecordError::SalesRecordNotFound)?;
        if record.status == "voided" {
            return Err(SalesRecordError::SalesRecordVoided);
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
struct SalesRecordReferenceInput {
    customer_id: Uuid,
    department_id: Uuid,
    system_id: Uuid,
    store_id: Uuid,
    handler_user_id: Uuid,
    expert_user_id: Option<Uuid>,
    expert_department_id: Option<Uuid>,
    consultant_user_id: Option<Uuid>,
    consultant_department_id: Option<Uuid>,
    doctor_user_id: Option<Uuid>,
}

#[derive(Debug, Error)]
pub enum SalesRecordError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("sales record was not found")]
    SalesRecordNotFound,
    #[error("operation count was not found")]
    OperationCountNotFound,
    #[error("operation usage was not found")]
    OperationUsageNotFound,
    #[error("customer was not found")]
    CustomerNotFound,
    #[error("customer is disabled")]
    CustomerDisabled,
    #[error("department was not found")]
    DepartmentNotFound,
    #[error("system was not found")]
    SystemNotFound,
    #[error("store was not found")]
    StoreNotFound,
    #[error("product category was not found")]
    ProductCategoryNotFound,
    #[error("product category is disabled")]
    ProductCategoryDisabled,
    #[error("{field} user was not found")]
    UserNotFound { field: &'static str },
    #[error("{field} user is disabled")]
    ReferencedUserDisabled { field: &'static str },
    #[error("system does not belong to the selected department")]
    SystemDepartmentMismatch,
    #[error("store does not belong to the selected system")]
    StoreSystemMismatch,
    #[error("sales record batch must include at least one record")]
    EmptyBatch,
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
    #[error("{field} must be less than or equal to 9999999999.99")]
    MoneyTooLarge { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidCountMinimum { field: &'static str, minimum: i32 },
    #[error("operation_total_count is required for this content category")]
    OperationTotalCountRequired,
    #[error("operation_total_count is not allowed for this content category")]
    OperationTotalCountNotAllowed,
    #[error("expert_user_id and expert_department_id are required for expert consultation")]
    MissingExpertFields,
    #[error("expert_user_id and expert_department_id must be empty for self sale")]
    UnexpectedExpertFields,
    #[error(
        "sales record content category cannot switch between operation-count and non-operation-count categories"
    )]
    ContentCategoryOperationCountMismatch,
    #[error("sales record is voided")]
    SalesRecordVoided,
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
            Self::OperationCountNotFound => "operation_count_not_found",
            Self::OperationUsageNotFound => "operation_usage_not_found",
            Self::CustomerNotFound => "customer_not_found",
            Self::CustomerDisabled => "customer_disabled",
            Self::DepartmentNotFound => "department_not_found",
            Self::SystemNotFound => "system_not_found",
            Self::StoreNotFound => "store_not_found",
            Self::ProductCategoryNotFound => "product_category_not_found",
            Self::ProductCategoryDisabled => "product_category_disabled",
            Self::UserNotFound { .. } => "user_not_found",
            Self::ReferencedUserDisabled { .. } => "referenced_user_disabled",
            Self::SystemDepartmentMismatch => "system_department_mismatch",
            Self::StoreSystemMismatch => "store_system_mismatch",
            Self::OperationCountInsufficient => "operation_count_insufficient",
            Self::OperationCountBelowUsed => "operation_count_below_used",
            Self::SalesRecordHasActiveUsages => "sales_record_has_active_operation_usages",
            Self::SalesRecordVoided => "sales_record_voided",
            Self::OperationCountVoided => "operation_count_voided",
            Self::OperationUsageVoided => "operation_usage_voided",
            Self::EmptyBatch
            | Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidEnumValue { .. }
            | Self::InvalidMoney { .. }
            | Self::NegativeMoney { .. }
            | Self::MoneyTooLarge { .. }
            | Self::InvalidCountMinimum { .. }
            | Self::OperationTotalCountRequired
            | Self::OperationTotalCountNotAllowed
            | Self::MissingExpertFields
            | Self::UnexpectedExpertFields
            | Self::ContentCategoryOperationCountMismatch
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
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

fn nullable_uuid_change(
    _field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Option<Uuid>>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => Ok(Some(Some(value))),
    }
}

fn required_date_change(
    field: &'static str,
    value: PatchField<NaiveDate>,
) -> Result<Option<NaiveDate>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn enum_text_change(
    field: &'static str,
    value: PatchField<String>,
    parser: fn(&'static str, &str) -> Result<&'static str, EnumParseError>,
) -> Result<Option<String>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => parser(field, &value)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn money_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<Decimal>, SalesRecordError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SalesRecordError::MissingRequiredField { field }),
        PatchField::Value(value) => parse_money(field, Some(value)).map(Some),
    }
}

async fn operation_usage_changes(
    request: UpdateOperationUsageRequest,
) -> Result<OperationUsageChanges, SalesRecordError> {
    Ok(OperationUsageChanges {
        operated_at: required_datetime_change("operated_at", request.operated_at)?,
        operator_user_id: required_uuid_change("operator_user_id", request.operator_user_id)?,
        doctor_user_id: nullable_uuid_change("doctor_user_id", request.doctor_user_id)?,
        operation_count: count_change("operation_count", request.operation_count)?,
        remark: nullable_text_change("remark", request.remark, MAX_REMARK_LENGTH)?,
        status: None,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        dto::sales_records::{
            CreateOperationUsageRequest, CreateSalesRecordRequest, ListOperationCountsQuery,
            ListOperationUsagesQuery, ListSalesRecordsQuery, UpdateOperationCountRequest,
        },
        repositories::{
            customers::{CustomerRepository, NewCustomer},
            departments::DepartmentRepository,
            product_categories::ProductCategoryRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::path::PathBuf;

    struct Harness {
        users: UserRepository,
        departments: DepartmentRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
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
            let departments = DepartmentRepository::new(db.clone());
            let systems = SystemRepository::new(db.clone());
            let stores = StoreRepository::new(db.clone());
            let customers = CustomerRepository::new(db.clone());
            let categories = ProductCategoryRepository::new(db.clone());
            let sales_records = SalesRecordRepository::new(db);
            let service = SalesRecordService::new(
                sales_records,
                customers.clone(),
                departments.clone(),
                systems.clone(),
                stores.clone(),
                categories.clone(),
                users.clone(),
            );

            Self {
                users,
                departments,
                systems,
                stores,
                customers,
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

        async fn scope(&self, name: &str) -> (Uuid, Uuid, Uuid) {
            let now = Utc::now();
            let department = self
                .departments
                .insert_department(Uuid::new_v4(), "manual", name, name, None, now)
                .await
                .expect("department should be created");
            let system = self
                .systems
                .create_system(
                    NewSystem {
                        name: name.to_string(),
                        department_id: department.id,
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

            (department.id, system.id, store.id)
        }

        async fn customer(
            &self,
            creator_user_id: Uuid,
            department_id: Uuid,
            system_id: Uuid,
            store_id: Uuid,
        ) -> Uuid {
            self.customers
                .create_customer(
                    NewCustomer {
                        name: "Alice".to_string(),
                        creator_user_id,
                        department_id,
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
    }

    fn sales_request(
        customer_id: Uuid,
        department_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
        category_id: Uuid,
        handler_user_id: Uuid,
        operation_total_count: Option<i32>,
    ) -> CreateSalesRecordRequest {
        CreateSalesRecordRequest {
            customer_id,
            department_id,
            sale_date: Utc
                .with_ymd_and_hms(2026, 7, 8, 0, 0, 0)
                .unwrap()
                .date_naive(),
            deal_status: "closed".to_string(),
            customer_type: "new".to_string(),
            deal_type: "non_salon".to_string(),
            content_category_id: category_id,
            handler_user_id,
            paid_amount: "100.50".to_string(),
            unpaid_amount: "0".to_string(),
            system_id,
            store_id,
            collaboration_type: "self_sale".to_string(),
            expert_user_id: None,
            expert_department_id: None,
            consultant_user_id: None,
            consultant_department_id: None,
            doctor_user_id: None,
            operation_total_count,
        }
    }

    async fn create_operation_sales_record(h: &Harness, total_count: i32) -> Uuid {
        let handler = h.user("handler").await;
        let (department_id, system_id, store_id) = h.scope("scope-a").await;
        let customer_id = h
            .customer(handler, department_id, system_id, store_id)
            .await;
        let category_id = h.category(true).await;
        h.service
            .create_sales_record_batch(CreateSalesRecordBatchRequest {
                records: vec![sales_request(
                    customer_id,
                    department_id,
                    system_id,
                    store_id,
                    category_id,
                    handler,
                    Some(total_count),
                )],
            })
            .await
            .expect("sales record should be created")
            .sales_records[0]
            .id
    }

    #[tokio::test]
    async fn creates_batch_and_lists_records_with_counts() {
        let h = Harness::new().await;
        let handler = h.user("handler").await;
        let (department_id, system_id, store_id) = h.scope("scope-a").await;
        let customer_id = h
            .customer(handler, department_id, system_id, store_id)
            .await;
        let product_category = h.category(false).await;
        let operation_category = h.category(true).await;

        let response = h
            .service
            .create_sales_record_batch(CreateSalesRecordBatchRequest {
                records: vec![
                    sales_request(
                        customer_id,
                        department_id,
                        system_id,
                        store_id,
                        product_category,
                        handler,
                        None,
                    ),
                    sales_request(
                        customer_id,
                        department_id,
                        system_id,
                        store_id,
                        operation_category,
                        handler,
                        Some(5),
                    ),
                ],
            })
            .await
            .expect("batch should be created");

        assert_eq!(response.sales_records.len(), 2);
        assert!(
            response
                .sales_records
                .iter()
                .all(|record| record.record_group_id == Some(response.record_group_id))
        );
        assert!(response.sales_records[0].operation_count.is_none());
        assert_eq!(
            response.sales_records[1]
                .operation_count
                .as_ref()
                .map(|count| count.total_count),
            Some(5)
        );

        let list = h
            .service
            .list_sales_records(ListSalesRecordsQuery {
                record_group_id: Some(response.record_group_id),
                page_number: Some(1),
                page_size: Some(20),
                ..ListSalesRecordsQuery::default()
            })
            .await
            .expect("records should list");
        assert_eq!(list.total_count, 2);
    }

    #[tokio::test]
    async fn validates_sales_record_inputs() {
        let h = Harness::new().await;
        let handler = h.user("handler").await;
        let (department_id, system_id, store_id) = h.scope("scope-a").await;
        let customer_id = h
            .customer(handler, department_id, system_id, store_id)
            .await;
        let product_category = h.category(false).await;
        let operation_category = h.category(true).await;

        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest { records: vec![] })
                .await,
            Err(SalesRecordError::EmptyBatch)
        ));
        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest {
                    records: vec![sales_request(
                        customer_id,
                        department_id,
                        system_id,
                        store_id,
                        operation_category,
                        handler,
                        None,
                    )],
                })
                .await,
            Err(SalesRecordError::OperationTotalCountRequired)
        ));
        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest {
                    records: vec![sales_request(
                        customer_id,
                        department_id,
                        system_id,
                        store_id,
                        product_category,
                        handler,
                        Some(1),
                    )],
                })
                .await,
            Err(SalesRecordError::OperationTotalCountNotAllowed)
        ));

        let mut invalid = sales_request(
            customer_id,
            department_id,
            system_id,
            store_id,
            product_category,
            handler,
            None,
        );
        invalid.paid_amount = "-0.01".to_string();
        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest {
                    records: vec![invalid],
                })
                .await,
            Err(SalesRecordError::NegativeMoney { .. })
        ));

        let mut invalid = sales_request(
            customer_id,
            department_id,
            system_id,
            store_id,
            product_category,
            handler,
            None,
        );
        invalid.deal_status = "pending".to_string();
        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest {
                    records: vec![invalid],
                })
                .await,
            Err(SalesRecordError::InvalidEnumValue {
                field: "deal_status",
                ..
            })
        ));

        let mut invalid = sales_request(
            customer_id,
            department_id,
            system_id,
            store_id,
            product_category,
            handler,
            None,
        );
        invalid.expert_user_id = Some(handler);
        assert!(matches!(
            h.service
                .create_sales_record_batch(CreateSalesRecordBatchRequest {
                    records: vec![invalid],
                })
                .await,
            Err(SalesRecordError::UnexpectedExpertFields)
        ));
    }

    #[tokio::test]
    async fn operation_usage_lifecycle_maintains_used_count() {
        let h = Harness::new().await;
        let sales_record_id = create_operation_sales_record(&h, 3).await;
        let operator = h.user("operator").await;
        let operated_at = Utc.with_ymd_and_hms(2026, 7, 8, 9, 0, 0).unwrap();

        let usage = h
            .service
            .create_operation_usage(CreateOperationUsageRequest {
                sales_record_id,
                operated_at,
                operator_user_id: operator,
                doctor_user_id: None,
                operation_count: 1,
                remark: Some(" first ".to_string()),
            })
            .await
            .expect("usage should be created");
        assert_eq!(usage.operation_count, 1);
        assert_eq!(
            h.service
                .operation_count_detail(sales_record_id)
                .await
                .expect("count should load")
                .used_count,
            1
        );

        let updated = h
            .service
            .update_operation_usage(
                usage.id,
                serde_json::from_value(json!({"operation_count": 2, "remark": null}))
                    .expect("update request should deserialize"),
            )
            .await
            .expect("usage should update");
        assert_eq!(updated.operation_count, 2);
        assert_eq!(updated.remark, None);
        assert_eq!(
            h.service
                .operation_count_detail(sales_record_id)
                .await
                .expect("count should load")
                .used_count,
            2
        );

        h.service
            .create_operation_usage(CreateOperationUsageRequest {
                sales_record_id,
                operated_at,
                operator_user_id: operator,
                doctor_user_id: None,
                operation_count: 1,
                remark: None,
            })
            .await
            .expect("second usage should fit");
        assert!(matches!(
            h.service
                .create_operation_usage(CreateOperationUsageRequest {
                    sales_record_id,
                    operated_at,
                    operator_user_id: operator,
                    doctor_user_id: None,
                    operation_count: 1,
                    remark: None,
                })
                .await,
            Err(SalesRecordError::OperationCountInsufficient)
        ));
        assert!(matches!(
            h.service
                .update_operation_count(
                    sales_record_id,
                    UpdateOperationCountRequest { total_count: 2 }
                )
                .await,
            Err(SalesRecordError::OperationCountBelowUsed)
        ));

        h.service
            .void_operation_usage(usage.id)
            .await
            .expect("usage should void");
        assert_eq!(
            h.service
                .operation_count_detail(sales_record_id)
                .await
                .expect("count should load")
                .used_count,
            1
        );
        h.service
            .delete_operation_usage(usage.id)
            .await
            .expect("voided usage should delete without another decrement");
        assert_eq!(
            h.service
                .operation_count_detail(sales_record_id)
                .await
                .expect("count should load")
                .used_count,
            1
        );
    }

    #[tokio::test]
    async fn sales_record_void_and_delete_require_no_active_usages() {
        let h = Harness::new().await;
        let sales_record_id = create_operation_sales_record(&h, 2).await;
        let operator = h.user("operator").await;
        let usage = h
            .service
            .create_operation_usage(CreateOperationUsageRequest {
                sales_record_id,
                operated_at: Utc::now(),
                operator_user_id: operator,
                doctor_user_id: None,
                operation_count: 1,
                remark: None,
            })
            .await
            .expect("usage should be created");

        assert!(matches!(
            h.service.void_sales_record(sales_record_id).await,
            Err(SalesRecordError::SalesRecordHasActiveUsages)
        ));
        assert!(matches!(
            h.service.delete_sales_record(sales_record_id).await,
            Err(SalesRecordError::SalesRecordHasActiveUsages)
        ));

        h.service
            .void_operation_usage(usage.id)
            .await
            .expect("usage should void");
        let voided = h
            .service
            .void_sales_record(sales_record_id)
            .await
            .expect("sales record should void");
        assert_eq!(voided.status, "voided");
        assert_eq!(
            voided
                .operation_count
                .as_ref()
                .map(|count| count.status.as_str()),
            Some("voided")
        );
        h.service
            .delete_sales_record(sales_record_id)
            .await
            .expect("voided sales record should delete");
        assert!(matches!(
            h.service.sales_record_detail(sales_record_id).await,
            Err(SalesRecordError::SalesRecordNotFound)
        ));
    }

    #[tokio::test]
    async fn validates_lists_updates_and_category_count_shape() {
        let h = Harness::new().await;
        let sales_record_id = create_operation_sales_record(&h, 3).await;
        let product_category = h.category(false).await;

        assert!(matches!(
            h.service
                .update_sales_record(
                    sales_record_id,
                    serde_json::from_value(json!({"content_category_id": product_category}))
                        .expect("update request should deserialize"),
                )
                .await,
            Err(SalesRecordError::ContentCategoryOperationCountMismatch)
        ));

        let updated = h
            .service
            .update_sales_record(
                sales_record_id,
                serde_json::from_value(json!({"paid_amount": "200.00"}))
                    .expect("update request should deserialize"),
            )
            .await
            .expect("sales record should update");
        assert_eq!(updated.paid_amount, "200.00");

        let counts = h
            .service
            .list_operation_counts(ListOperationCountsQuery {
                sales_record_id: Some(sales_record_id),
                ..ListOperationCountsQuery::default()
            })
            .await
            .expect("counts should list");
        assert_eq!(counts.total_count, 1);

        let usages = h
            .service
            .list_operation_usages(ListOperationUsagesQuery {
                sales_record_id: Some(sales_record_id),
                page_number: Some(1),
                page_size: Some(20),
                ..ListOperationUsagesQuery::default()
            })
            .await
            .expect("usages should list");
        assert_eq!(usages.total_count, 0);

        assert!(matches!(
            h.service
                .list_sales_records(ListSalesRecordsQuery {
                    page_number: Some(0),
                    ..ListSalesRecordsQuery::default()
                })
                .await,
            Err(SalesRecordError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
    }
}
