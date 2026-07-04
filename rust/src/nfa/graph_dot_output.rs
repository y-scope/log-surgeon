use crate::nfa::CaptureTag;
use crate::nfa::NfaState;
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
					for &target in transitions.iter() {
						let src_scc: usize = tarjan.vertices[state.idx.0].scc;
						let dst_scc: usize = tarjan.vertices[target.0].scc;
						let colour: &str = if src_scc == dst_scc { " [color=\"red\"]" } else { "" };
						lines.push_str(&format!(
							"\t{} -> {} [label=\"\u{03b5}\"]{colour};\n",
							state.idx, target
						));
					}
				},
				Transitions::Tagged { tag, positive, target } => {
					let src_scc: usize = tarjan.vertices[state.idx.0].scc;
					let dst_scc: usize = tarjan.vertices[target.0].scc;
					let colour: &str = if src_scc == dst_scc { " [color=\"red\"]" } else { "" };
					let mut capture: String = match tag {
						CaptureTag::StartCapture(sub_rule) => {
							format!("start({})", sub_rule.qualified_name)
						},
						CaptureTag::StopCapture(sub_rule) => {
							format!("stop({})", sub_rule.qualified_name)
						},
					};
					if !positive {
						capture = format!("-{capture}");
					}
					lines.push_str(&format!(
						"\t{} -> {} [label=\"{capture}\"]{colour}\n",
						state.idx, target
					));
				},
			}
			lines.push('\n');
		}

		lines.push_str("}");

		lines
	}
}
