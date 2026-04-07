mod collect;
mod pipeline;
mod rules;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_false_positives;

pub(super) use collect::collect_raw_processes;
pub(super) use pipeline::classify_process_list;
