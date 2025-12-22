use bevy::ecs::schedule::{
    IntoScheduleConfigs, Schedule, ScheduleBuildSettings, ScheduleLabel, SystemSet,
};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct GlaceRender;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GlaceRenderSystems {
    Clear,
    StartMainPass,
    EndMainPass,
    Blit,
    EguiPass,
    Submit,
}

impl GlaceRender {
    pub fn schedule() -> Schedule {
        use GlaceRenderSystems::*;

        let mut schedule = Schedule::new(Self);
        schedule.set_build_settings(ScheduleBuildSettings {
            auto_insert_apply_deferred: false,
            ..Default::default()
        });
        schedule
            .configure_sets((Clear, StartMainPass, EndMainPass, Blit, EguiPass, Submit).chain());
        schedule
    }
}
