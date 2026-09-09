use crate::{
    plugin_catalog::PluginInfo,
    plugin_details::{DetailTarget, ParameterDetail, PluginDetails},
    plugin_plot::Input,
};
use oxitone_graph::PluginDescriptor;

pub fn details(descriptor: PluginDescriptor) -> PluginDetails {
    let state = match descriptor.plugin_id.as_str() {
        "oxitone.slicer" => Some(serde_json::json!({"slices":{"grid":8},"triggerNote":36})),
        "oxitone.multisampler" => Some(
            serde_json::json!({"regions":[{"resource":"Soft","rootKey":60,"keyRange":[48,72],"velocityRange":[1,64]},{"resource":"Loud","rootKey":60,"keyRange":[48,72],"velocityRange":[65,127]}]}),
        ),
        _ => None,
    };
    PluginDetails {
        target: DetailTarget::Instrument("test".into()),
        owner_name: "Test".into(),
        name: descriptor.plugin_id.clone(),
        parameters: descriptor
            .parameters
            .iter()
            .map(|s| ParameterDetail {
                spec: s.clone(),
                value: s.default,
                explicit: false,
                host: false,
                automation: vec![],
            })
            .collect(),
        info: PluginInfo {
            descriptor,
            library: None,
        },
        settings: vec![],
        resources: vec![],
        state,
        source: serde_json::json!({}),
    }
}
#[test]
fn every_bundled_plugin_has_complete_controls_and_a_visual() {
    let engine = crate::tests::engine();
    let project = engine.current.unwrap();
    let descriptors: Vec<_> = oxitone_mixer::builtin_effect_plugins()
        .iter()
        .map(|p| p.descriptor().clone())
        .chain(
            oxitone_instruments::builtin_plugins()
                .iter()
                .map(|p| p.descriptor().clone()),
        )
        .collect();
    assert_eq!(descriptors.len(), 30);
    for descriptor in descriptors {
        let d = details(descriptor);
        let layout = crate::plugin_layout_builtin::panel(&d);
        crate::plugin_layout_validation::validate(&layout, &d.info.descriptor).unwrap();
        let controls: Vec<_> = layout
            .pages
            .iter()
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.controls)
            .collect();
        let bindings: Vec<_> = controls.iter().flat_map(|c| c.bindings()).collect();
        for spec in &d.info.descriptor.parameters {
            assert!(
                bindings.contains(&spec.id.as_str()),
                "{} missing {}",
                d.name,
                spec.id
            );
        }
        if d.name == "oxitone.wavetable" {
            assert!(controls.iter().any(|c| c.visual()));
        } else {
            let plots = crate::plugin_plot::build(&d, &project);
            assert!(!plots.is_empty(), "{} has no plot", d.name);
            assert!(
                plots
                    .iter()
                    .any(|p| !p.traces.is_empty() || !p.regions.is_empty()),
                "{}",
                d.name
            );
        }
    }
}
#[test]
fn plots_are_finite_and_bounded_at_every_parameter_extreme() {
    let engine = crate::tests::engine();
    let project = engine.current.unwrap();
    for plugin in oxitone_mixer::builtin_effect_plugins() {
        let mut d = details(plugin.descriptor().clone());
        for index in 0..d.parameters.len() {
            let spec = d.parameters[index].spec.clone();
            for value in [spec.min, spec.max] {
                d.parameters[index].value = value;
                let plots = crate::plugin_plot::build(&d, &project);
                assert!(plots.len() <= 2);
                for plot in plots {
                    assert!(plot.traces.len() <= 16);
                    for trace in plot.traces {
                        assert!(trace.points.len() <= 4096);
                        assert!(
                            trace
                                .points
                                .iter()
                                .all(|(x, y)| x.is_finite() && y.is_finite()),
                            "{} {}",
                            d.name,
                            spec.id
                        );
                    }
                }
            }
            d.parameters[index].value = spec.default;
        }
    }
}
#[test]
fn filter_units_match_the_dsp_and_compressor_preserves_unity_ratio() {
    let mut d = details(
        oxitone_mixer::builtin_effect_plugins()
            .into_iter()
            .find(|p| p.descriptor().plugin_id == "oxitone.filter")
            .unwrap()
            .descriptor()
            .clone(),
    );
    d.parameters
        .iter_mut()
        .find(|p| p.spec.id == "resonance")
        .unwrap()
        .value = 1.;
    let p = Input {
        details: &d,
        sample_rate: 48000.,
    };
    assert!(crate::plugin_filter_plot::filter_db(&p, 1000.).abs() < 1e-8);
    assert!(crate::plugin_filter_plot::filter_db(&p, 10000.) < -35.);
    let mut d = details(
        oxitone_mixer::builtin_effect_plugins()
            .into_iter()
            .find(|p| p.descriptor().plugin_id == "oxitone.compressor")
            .unwrap()
            .descriptor()
            .clone(),
    );
    d.parameters
        .iter_mut()
        .find(|p| p.spec.id == "ratio")
        .unwrap()
        .value = 1.;
    let p = Input {
        details: &d,
        sample_rate: 48000.,
    };
    for level in [-72., -18., 0., 12.] {
        assert!((crate::plugin_dynamics_plot::transfer(&p, level, 0) - level).abs() < 1e-10);
    }
}
#[test]
fn parameter_drag_uses_descriptor_mapping_and_integer_enums() {
    let d = details(oxitone_instruments::sampler::descriptor().clone());
    for parameter in d.parameters {
        assert_eq!(
            crate::plugin_edit::from_fraction(&parameter.spec, 0.),
            parameter.spec.min
        );
        assert!(
            (crate::plugin_edit::from_fraction(&parameter.spec, 1.) - parameter.spec.max).abs()
                < 1e-8
        );
        let value = crate::plugin_edit::from_fraction(&parameter.spec, 0.37);
        if parameter.spec.unit == oxitone_core::wire::ParameterUnit::Enum {
            assert_eq!(value.fract(), 0.);
        }
    }
}
#[test]
fn delay_shows_only_the_time_parameter_used_by_the_engine() {
    let mut d = details(
        oxitone_mixer::builtin_effect_plugins()
            .into_iter()
            .find(|p| p.descriptor().plugin_id == "oxitone.delay")
            .unwrap()
            .descriptor()
            .clone(),
    );
    assert!(crate::plugin_time_plot::active_parameter(&d, "timeBeats"));
    assert!(!crate::plugin_time_plot::active_parameter(
        &d,
        "timeSeconds"
    ));
    d.parameters
        .iter_mut()
        .find(|p| p.spec.id == "timeSeconds")
        .unwrap()
        .explicit = true;
    assert!(!crate::plugin_time_plot::active_parameter(&d, "timeBeats"));
    assert!(crate::plugin_time_plot::active_parameter(&d, "timeSeconds"));
}
#[test]
fn extreme_frequency_translation_remains_visible() {
    let mut d = details(
        oxitone_mixer::builtin_effect_plugins()
            .into_iter()
            .find(|p| p.descriptor().plugin_id == "oxitone.frequency-shifter")
            .unwrap()
            .descriptor()
            .clone(),
    );
    for value in [-5000., 5000.] {
        d.parameters
            .iter_mut()
            .find(|p| p.spec.id == "shiftHz")
            .unwrap()
            .value = value;
        let p = Input {
            details: &d,
            sample_rate: 48000.,
        };
        let plot = crate::plugin_color_plot::build(&p).remove(0);
        for trace in plot.traces.iter().filter(|t| !t.label.is_empty()) {
            assert!(trace.points.first().unwrap().1 - trace.points.last().unwrap().1 > 0.2);
        }
    }
}
#[test]
#[ignore = "release source-visual model benchmark; no device or GPU timing"]
fn benchmark_builtin_plots() {
    let engine = crate::tests::engine();
    let project = engine.current.unwrap();
    let details: Vec<_> = oxitone_mixer::builtin_effect_plugins()
        .iter()
        .map(|p| details(p.descriptor().clone()))
        .collect();
    let mut samples = vec![];
    for i in 0..1100 {
        let start = std::time::Instant::now();
        for d in &details {
            std::hint::black_box(crate::plugin_plot::build(d, &project));
        }
        if i >= 100 {
            samples.push(start.elapsed().as_secs_f64() * 1e6);
        }
    }
    samples.sort_by(f64::total_cmp);
    eprintln!("26 effect diagrams: p95 {:.3} µs; p99 {:.3} µs (100 warmup / 1000 samples, source model only)",samples[950],samples[990]);
}
