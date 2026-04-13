use crate::error::WikiError;
use crate::link_index::LinkIndex;
use crate::page::PageId;
use crate::wiki::Wiki;

/// Run `refs to <page>`: list all pages that link to the given page.
pub fn refs_to(wiki: &Wiki, page_name: &str) -> Result<(), WikiError> {
    let page_id = PageId::from(page_name);
    let index = LinkIndex::build(wiki)?;

    let mut sources = index.inbound(&page_id);
    sources.sort();

    for source in &sources {
        if let Some(entry) = wiki.get(source) {
            println!("{} -> {page_name}", entry.rel_path.display());
        } else {
            println!("{source} -> {page_name}");
        }
    }

    if sources.is_empty() {
        eprintln!("no inbound links to '{page_name}'");
    }

    Ok(())
}

/// Run `refs from <page>`: list all pages the given page links to.
pub fn refs_from(wiki: &Wiki, page_name: &str) -> Result<(), WikiError> {
    let page_id = PageId::from(page_name);
    let index = LinkIndex::build(wiki)?;

    let outbound = index.outbound(&page_id);

    let mut targets: Vec<&str> = outbound.iter().map(|wl| wl.page.as_str()).collect();
    targets.sort();
    targets.dedup();

    for target in &targets {
        println!("{page_name} -> {target}");
    }

    if targets.is_empty() {
        eprintln!("no outbound links from '{page_name}'");
    }

    Ok(())
}

/// Run `refs graph`: dump the full link graph.
pub fn refs_graph(wiki: &Wiki) -> Result<(), WikiError> {
    let index = LinkIndex::build(wiki)?;
    let edges = index.all_edges();

    for (source, target) in &edges {
        println!("{source} -> {target}");
    }

    Ok(())
}
