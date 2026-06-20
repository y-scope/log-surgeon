use crate::nfa::CaptureTag;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::utils::TarjanSccs;

impl Tnfa {
	pub fn to_dot_output(&self) -> String {
		let mut lines: String = String::new();

		lines.push_str("digraph NFA {\n");

		lines.push_str("\trankdir=LR;\n");
		lines.push_str("\tnode [shape=circle];\n");
		lines.push('\n');

		let tarjan: TarjanSccs =
			TarjanSccs::tarjan_scc(&self.states, |state| state.transitions.successors().map(|idx| idx.0));
		for scc in tarjan.sccs.iter() {
			for &state in scc.iter() {
				let state: &NfaState = &self.states[state];
				let shape: &str = if state.is_accepting() {
					" [shape=doublecircle]"
				} else {
					""
				};
				let colour: &str = if scc.len() > 1 { " [color=\"red\"]" } else { "" };
				lines.push_str(&format!(
					"\t{} [label=\"{}: {:#}\"]{shape}{colour};\n",
					state.idx, state.idx, state.name
				));
			}
		}
		lines.push('\n');

		for state in self.states.iter() {
			match &state.transitions {
				Transitions::Interval(transitions) => {
					for (interval, &target) in transitions.iter() {
						let src_scc: usize = tarjan.vertices[state.idx.0].scc;
						let dst_scc: usize = tarjan.vertices[target.0].scc;
						let colour: &str = if src_scc == dst_scc { " [color=\"red\"]" } else { "" };
						lines.push_str(&format!(
							"\t{} -> {} [label=\"{}\"]{colour}\n",
							state.idx,
							target,
							if interval.start() == interval.end() {
								char::from_u32(interval.start()).unwrap().to_string()
							} else {
								format!(
									"{} - {}",
									char::from_u32(interval.start())
										.as_ref()
										.map(ToString::to_string)
										.map(|s| if s == "\0" { "null".to_owned() } else { s })
										.unwrap_or(format!("u{{{:x}}}", interval.start())),
									char::from_u32(interval.end())
										.as_ref()
										.map(ToString::to_string)
										.map(|s| if s == "\0" { "null".to_owned() } else { s })
										.unwrap_or(format!("u{{{:x}}}", interval.end())),
								)
							}
						))
					}
				},
				Transitions::Spontaneous(transitions) => {
					for transition in transitions.iter() {
						lines.push_str(&format!("\t{} -> {} [label=\"", state.idx, transition.target));
						match &transition.kind {
							SpontaneousTransitionKind::Epsilon => {
								lines.push('\u{03b5}');
							},
							SpontaneousTransitionKind::Positive(CaptureTag::StartCapture(sub_rule)) => {
								lines.push_str(&format!("start({})", sub_rule.qualified_name));
							},
							SpontaneousTransitionKind::Positive(CaptureTag::StopCapture(sub_rule)) => {
								lines.push_str(&format!("stop({})", sub_rule.qualified_name));
							},
							SpontaneousTransitionKind::Negative(
								CaptureTag::StartCapture(sub_rule) | CaptureTag::StopCapture(sub_rule),
							) => {
								lines.push('-');
								lines.push_str(&sub_rule.qualified_name);
							},
						}
						let src_scc: usize = tarjan.vertices[state.idx.0].scc;
						let dst_scc: usize = tarjan.vertices[transition.target.0].scc;
						let colour: &str = if src_scc == dst_scc { " [color=\"red\"]" } else { "" };
						lines.push_str(&format!("\"]{colour};\n"));
					}
				},
			}
			lines.push('\n');
		}

		lines.push_str("}");

		lines
	}
}
