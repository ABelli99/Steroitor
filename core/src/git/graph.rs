//! Corsie del grafo dei commit (stesso algoritmo di `src/lib/git/graph.ts`).
//! Ogni corsia "aspetta" l'hash del prossimo commit che la continua (il primo parent);
//! i parent aggiuntivi dei merge aprono o raggiungono altre corsie.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Half {
    Top,
    Bottom,
}

/// Tratto di mezza riga: da una corsia all'altra, nella metà alta o bassa della riga.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub half: Half,
    pub from: usize,
    pub to: usize,
    pub color: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphRow {
    pub lane: usize,
    pub color: usize,
    pub segments: Vec<Segment>,
    pub width: usize,
}

pub fn layout_graph<'a>(commits: impl IntoIterator<Item = (&'a str, &'a [String])>) -> Vec<GraphRow> {
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut colors: Vec<usize> = Vec::new();
    let mut next_color = 0;

    let mut open = |lanes: &mut Vec<Option<String>>, colors: &mut Vec<usize>, hash: &str| {
        let lane = lanes.iter().position(Option::is_none).unwrap_or(lanes.len());
        if lane == lanes.len() {
            lanes.push(None);
            colors.push(0);
        }
        lanes[lane] = Some(hash.to_owned());
        colors[lane] = next_color;
        next_color += 1;
        lane
    };

    commits
        .into_iter()
        .map(|(hash, parents)| {
            let lane = match lanes.iter().position(|expected| expected.as_deref() == Some(hash)) {
                Some(existing) => existing,
                None => open(&mut lanes, &mut colors, hash),
            };
            let mut segments = Vec::new();
            for (index, expected) in lanes.iter().enumerate() {
                if let Some(expected) = expected {
                    let to = if expected == hash { lane } else { index };
                    segments.push(Segment { half: Half::Top, from: index, to, color: colors[index] });
                }
            }
            for (index, expected) in lanes.iter_mut().enumerate() {
                if expected.as_deref() == Some(hash) && index != lane {
                    *expected = None;
                }
            }

            let mut from_node = Vec::new();
            let (first, others) = parents.split_first().map_or((None, &[][..]), |(first, others)| (Some(first), others));
            lanes[lane] = first.cloned();
            if first.is_some() {
                from_node.push(lane);
            }
            for parent in others {
                let target = match lanes.iter().position(|expected| expected.as_deref() == Some(parent.as_str())) {
                    Some(target) => target,
                    None => open(&mut lanes, &mut colors, parent),
                };
                from_node.push(target);
            }

            for (index, expected) in lanes.iter().enumerate() {
                if expected.is_some() {
                    let from = if from_node.contains(&index) { lane } else { index };
                    segments.push(Segment { half: Half::Bottom, from, to: index, color: colors[index] });
                }
            }
            while lanes.last().is_some_and(Option::is_none) {
                lanes.pop();
                colors.pop();
            }

            let width = segments.iter().flat_map(|segment| [segment.from, segment.to]).chain([lane]).max().unwrap_or(0) + 1;
            GraphRow { lane, color: colors.get(lane).copied().unwrap_or(0), segments, width }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(commits: &[(&str, &[&str])]) -> Vec<GraphRow> {
        let owned: Vec<(String, Vec<String>)> = commits.iter().map(|(hash, parents)| (hash.to_string(), parents.iter().map(|p| p.to_string()).collect())).collect();
        layout_graph(owned.iter().map(|(hash, parents)| (hash.as_str(), parents.as_slice())))
    }

    #[test]
    fn keeps_a_linear_history_in_one_lane() {
        let rows = layout(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        assert!(rows.iter().all(|row| row.lane == 0 && row.width == 1));
        assert!(rows[2].segments.iter().all(|segment| segment.half == Half::Top));
    }

    #[test]
    fn opens_a_second_lane_for_a_merge_and_joins_it_back() {
        // m unisce f (feature) in b; f parte da a, come b.
        let rows = layout(&[("m", &["b", "f"]), ("f", &["a"]), ("b", &["a"]), ("a", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[2].lane, 0);
        assert_eq!(rows[0].width, 2);
        // Il merge parte dal nodo verso la corsia 1.
        assert!(rows[0].segments.contains(&Segment { half: Half::Bottom, from: 0, to: 1, color: rows[1].color }));
        // a chiude entrambe le corsie: la 1 piega verso la 0.
        assert!(rows[3].segments.iter().any(|segment| segment.half == Half::Top && segment.from == 1 && segment.to == 0));
    }
}
