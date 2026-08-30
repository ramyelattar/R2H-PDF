mod ai_core;
mod annotation_core;
mod bentopdf_runtime;
pub mod content_editing;
pub mod document_core;
pub mod document_engine_host;
mod editing_core;
mod engineering;
pub mod license;
mod performance_reliability_core;
mod reports;
mod search_core;
mod release_smoke;
mod project_persistence;
mod shell_persistence;

use ai_core::AiEngine;
use ai_core::LocalAiState;
use ai_core::RagState;
use ai_core::ActionPlannerState;
use annotation_core::{AnnotationCoreState, AnnotationEngine};
use document_core::ipc::{
    close_session, doc_close, doc_diagnostics, doc_extract_all_text, doc_extract_text,
    doc_incremental_save, doc_navigate, doc_open, doc_recover, doc_render, doc_list_form_fields,
    get_session_state, go_to_page, open_pdf, render_page, set_zoom,
    ocr_check_availability, ocr_get_status, ocr_cancel, ocr_has_result, ocr_get_page_text,
    ocr_run_page, ocr_get_all_texts,
    pdf_create_form_field, pdf_delete_form_field, pdf_update_form_field_properties,
    pdf_compare_documents, pdf_get_compare_result, pdf_clear_compare_result,
    report_export_compare_review, ai_review_compare_result,
    pdf_create_ocr_editable_overlays, pdf_create_ocr_text_layer,
};
use document_core::compare::CompareState;
use document_core::export::doc_export;
use document_core::DocumentCoreState;
use document_core::OcrState;
use editing_core::{EditingCoreState, EditingEngine};
use license::license_get_status;
use performance_reliability_core::ipc::{
    perf_complete_job, perf_create_recovery_checkpoint, perf_diagnostics,
    perf_evaluate_memory, perf_get_benchmark_plan, perf_get_memory_model,
    perf_get_recovery_architecture, perf_get_strategy, perf_get_test_corpus_strategy,
    perf_list_jobs, perf_mark_job_running, perf_record_autosave, perf_restore_after_crash,
    perf_schedule_job, perf_submit_benchmark_result, perf_validate_autosave,
};
use performance_reliability_core::PerformanceReliabilityState;
use search_core::{SearchCoreState, SearchEngine};
use project_persistence::{project_load, project_save};
use shell_persistence::{
    library_load, library_record_export, library_record_open, library_record_project,
    library_record_review,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let document_core_state = DocumentCoreState::new();
    let performance_reliability_state = PerformanceReliabilityState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(document_core_state)
        .manage(performance_reliability_state)
        .manage(bentopdf_runtime::BentoPdfRuntimeState::default())
        .manage(EditingCoreState::new(EditingEngine::new()))
        .manage(AnnotationCoreState::new(AnnotationEngine::new()))
        .manage(SearchCoreState::new(SearchEngine::new()))
        .manage(tokio::sync::Mutex::new(AiEngine::new()))
        .manage(OcrState::new())
        .manage(LocalAiState::new())
        .manage(RagState::new())
        .manage(ActionPlannerState::new())
        .manage(engineering::EngineeringState::new())
        .manage(content_editing::history::ContentEditHistoryState::new())
        .manage(CompareState::new())
        .invoke_handler(tauri::generate_handler![
            bentopdf_runtime::bentopdf_get_status,
            bentopdf_runtime::bentopdf_open,
            doc_open,
            doc_close,
            doc_render,
            doc_extract_text,
            doc_extract_all_text,
            doc_navigate,
            doc_incremental_save,
            doc_recover,
            doc_diagnostics,
            doc_list_form_fields,
            pdf_create_form_field,
            pdf_delete_form_field,
            pdf_update_form_field_properties,
            pdf_compare_documents,
            pdf_get_compare_result,
            pdf_clear_compare_result,
            report_export_compare_review,
            ai_review_compare_result,
            pdf_create_ocr_editable_overlays,
            pdf_create_ocr_text_layer,
            open_pdf,
            render_page,
            go_to_page,
            set_zoom,
            get_session_state,
            close_session,
            perf_get_benchmark_plan,
            perf_submit_benchmark_result,
            perf_get_strategy,
            perf_get_memory_model,
            perf_evaluate_memory,
            perf_schedule_job,
            perf_mark_job_running,
            perf_complete_job,
            perf_list_jobs,
            perf_record_autosave,
            perf_validate_autosave,
            perf_create_recovery_checkpoint,
            perf_restore_after_crash,
            perf_get_recovery_architecture,
            perf_get_test_corpus_strategy,
            perf_diagnostics,
            editing_core::ipc::edit_apply_transaction,
            editing_core::ipc::edit_undo,
            editing_core::ipc::edit_redo,
            editing_core::ipc::edit_get_undo_redo_state,
            annotation_core::ipc::annot_create,
            annotation_core::ipc::annot_update,
            annotation_core::ipc::annot_delete,
            annotation_core::ipc::annot_list,
            annotation_core::ipc::annot_export_fdf,
            annotation_core::ipc::annot_export_json,
            annotation_core::ipc::annot_import_fdf,
            search_core::ipc::search_query,
            search_core::ipc::search_index_document,
            search_core::ipc::search_index_document_with_spans,
            search_core::ipc::search_get_index_status,
            search_core::ipc::search_clear_index,
            search_core::ipc::search_set_active_page,
            ai_core::ipc::ai_run_task,
            ai_core::ipc::ai_get_status,
            ai_core::ipc::ai_cancel_task,
            ai_core::ipc::ai_list_local_models,
            ai_core::ipc::ai_get_local_runtime_status,
            ai_core::ipc::ai_validate_local_model,
            ai_core::ipc::ai_generate_local,
            ai_core::ipc::ai_build_document_index,
            ai_core::ipc::ai_get_document_index_status,
            ai_core::ipc::ai_clear_document_index,
            ai_core::ipc::ai_search_document_semantic,
            ai_core::ipc::ai_ask_document_rag,
            ai_core::ipc::ai_plan_document_actions,
            ai_core::ipc::ai_get_action_batch,
            ai_core::ipc::ai_accept_action,
            ai_core::ipc::ai_reject_action,
            ai_core::ipc::ai_apply_action_batch,
            ai_core::ipc::ai_clear_action_batch,
            ai_core::ipc::ai_review_document,
            ai_core::ipc::ai_get_review_result,
            ai_core::ipc::ai_clear_review_result,
            ai_core::ipc::local_ai_get_status,
            ai_core::ipc::local_ai_validate,
            engineering::engineering_parse_units,
            engineering::engineering_calculate,
            engineering::engineering_get_page_texts,
            engineering::engineering_extract_load_schedule,
            engineering::engineering_get_load_rows,
            engineering::engineering_clear_load_rows,
            engineering::engineering_generate_findings,
            engineering::engineering_get_findings,
            engineering::engineering_get_calculation_trace,
            license_get_status,
            reports::report_export_review,
            reports::report_preview_html,
            content_editing::ipc::pdf_get_page_content_objects,
            content_editing::ipc::pdf_apply_native_text_edit,
            content_editing::ipc::pdf_preview_find_replace,
            content_editing::ipc::pdf_apply_find_replace,
            content_editing::ipc::pdf_replace_native_image,
            content_editing::ipc::pdf_delete_native_image,
            content_editing::ipc::pdf_move_native_image,
            content_editing::ipc::pdf_crop_native_image,
            content_editing::ipc::pdf_rotate_native_image,
            content_editing::ipc::pdf_list_content_edits,
            content_editing::ipc::pdf_revert_content_edit,
            content_editing::ipc::pdf_clear_content_edit_history,
            content_editing::ipc::pdf_cover_path_object,
            content_editing::ipc::pdf_prepare_text_block_edit,
            content_editing::ipc::pdf_apply_text_block_edit,
            content_editing::ipc::pdf_get_page_font_registry,
            content_editing::ipc::pdf_get_page_rotation,
            ocr_check_availability,
            ocr_get_status,
            ocr_cancel,
            ocr_has_result,
            ocr_get_page_text,
            ocr_run_page,
            ocr_get_all_texts,
            doc_export,
            project_load,
            project_save,
            library_load,
            library_record_open,
            library_record_project,
            library_record_export,
            library_record_review,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub fn run_release_smoke_cli(workflow: &str) -> i32 {
    match release_smoke::run_workflow(workflow) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("release smoke failed: {err}");
            1
        }
    }
}

pub fn run_license_smoke_cli(mode: &str) -> i32 {
    match license::run_license_smoke(mode) {
        Ok(report) => {
            match serde_json::to_string_pretty(&report) {
                Ok(json) => {
                    if let Ok(path) = std::env::var("R2H_LICENSE_SMOKE_OUTPUT") {
                        if let Err(err) = std::fs::write(&path, &json) {
                            eprintln!("license smoke report write failed at {path}: {err}");
                            return 1;
                        }
                    } else {
                        println!("{json}");
                    }
                }
                Err(err) => eprintln!("license smoke report serialization failed: {err}"),
            }
            if report.status == "PASS" { 0 } else { 1 }
        }
        Err(err) => {
            eprintln!("license smoke failed: {err}");
            1
        }
    }
}
