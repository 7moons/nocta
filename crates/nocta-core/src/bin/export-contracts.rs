use nocta_core::{error::CommandError, models::*};
use ts_rs::TS;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/generated/contracts.ts");
    let config = ts_rs::Config::default();
    let declarations = [
        AccountHealth::decl(&config),
        AccountPresence::decl(&config),
        Account::decl(&config),
        AccountDetails::decl(&config),
        TotpCode::decl(&config),
        Folder::decl(&config),
        DurationSummary::decl(&config),
        MetricErrors::decl(&config),
        MetricsSummary::decl(&config),
        LaunchMode::decl(&config),
        JoinStrategy::decl(&config),
        PresetTarget::decl(&config),
        LaunchPreset::decl(&config),
        Schedule::decl(&config),
        AntiAfkAction::decl(&config),
        LaunchPacing::decl(&config),
        ArrangeMonitors::decl(&config),
        ProcessPriority::decl(&config),
        ClientProfile::decl(&config),
        ClientStage::decl(&config),
        ClickPoint::decl(&config),
        SystemMemory::decl(&config),
        ServerRegion::decl(&config),
        AccountField::decl(&config),
        PrivacySetting::decl(&config),
        RestrictedOption::decl(&config),
        QuickLoginDevice::decl(&config),
        BefriendMode::decl(&config),
        BefriendSummary::decl(&config),
        Hotkeys::decl(&config),
        FastFlag::decl(&config),
        nocta_core::launch::client::fast_flags::FlagKind::decl(&config),
        nocta_core::launch::client::fast_flags::AllowedFlag::decl(&config),
        nocta_core::accounts::ExportFormat::decl(&config),
        LaunchTargetInput::decl(&config),
        LaunchTargetSummary::decl(&config),
        ServerKind::decl(&config),
        ServerRow::decl(&config),
        ServerListResponse::decl(&config),
        ServerListOptions::decl(&config),
        PlaceInfo::decl(&config),
        GameSearchResult::decl(&config),
        RobloxUser::decl(&config),
        UserPresence::decl(&config),
        FriendEntry::decl(&config),
        InstanceStatus::decl(&config),
        GameInstance::decl(&config),
        ExternalInstance::decl(&config),
        MultiInstanceState::decl(&config),
        LauncherState::decl(&config),
        GamePreset::decl(&config),
        LogPreferences::decl(&config),
        ClientUpdateMode::decl(&config),
        ClientGraphicsApi::decl(&config),
        InstalledClient::decl(&config),
        LatestClient::decl(&config),
        ClientInstallProgress::decl(&config),
        ClientState::decl(&config),
        AppConfig::decl(&config),
        LogEntry::decl(&config),
        RuntimeInfo::decl(&config),
        TaskKind::decl(&config),
        TaskStatus::decl(&config),
        AccountTask::decl(&config),
        AppSnapshot::decl(&config),
        ImportIssue::decl(&config),
        LegacySource::decl(&config),
        ImportReport::decl(&config),
        ImportProgress::decl(&config),
        FolderInput::decl(&config),
        AccountUpdate::decl(&config),
        CommandError::decl(&config),
    ];
    let mut source = String::from(
        "// Generated from nocta-core. Run bun run contracts; do not edit manually.\n\n",
    );
    for declaration in declarations {
        source.push_str("export ");
        source.push_str(&declaration);
        source.push_str("\n\n");
    }
    std::fs::create_dir_all(destination.parent().unwrap())?;
    std::fs::write(destination, source)?;
    Ok(())
}
