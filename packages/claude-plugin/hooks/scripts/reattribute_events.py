#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["neo4j"]
# ///
"""
Event Re-Attribution Script

Fixes misattributed events by:
1. Finding completed features with 0 events
2. Using feature timestamps and keywords to find related events
3. Creating LINKED_TO relationships to correct features

Run with --dry-run first to see what would be changed.
"""

import argparse
import re
from neo4j import GraphDatabase

DRIVER = GraphDatabase.driver("bolt://localhost:7687")


def extract_keywords(text: str) -> set[str]:
    """Extract meaningful keywords from text for matching."""
    if not text:
        return set()
    stop_words = {
        'the', 'a', 'an', 'is', 'are', 'to', 'of', 'in', 'for', 'on', 'with',
        'and', 'or', 'not', 'this', 'that', 'it', 'be', 'as', 'at', 'by',
        'from', 'has', 'have', 'had', 'do', 'does', 'did', 'will', 'would',
        'could', 'should', 'may', 'might', 'must', 'shall', 'can', 'add',
        'create', 'update', 'fix', 'implement', 'phase', 'step'
    }
    words = re.findall(r'\b[a-zA-Z][a-zA-Z0-9_]{2,}\b', text.lower())
    return {w for w in words if w not in stop_words}


def get_features_without_events() -> list[dict]:
    """Get completed features that have no events linked."""
    with DRIVER.session() as session:
        result = session.run('''
            MATCH (f:Feature {status: 'complete'})-[:BELONGS_TO]->(p:Project)
            WHERE NOT EXISTS {
                MATCH (e:Event)-[:LINKED_TO]->(f)
            }
            AND f.completed_at IS NOT NULL
            AND f.created_at IS NOT NULL
            RETURN f.id as id,
                   f.description as description,
                   f.created_at as created_at,
                   f.completed_at as completed_at,
                   p.path as project_path
            ORDER BY f.completed_at DESC
        ''')
        return [dict(r) for r in result]


def find_candidate_events(feature: dict, window_hours: int = 48) -> list[dict]:
    """
    Find events that could belong to this feature based on:
    1. Time window around feature creation/completion
    2. Keyword matching with feature description
    """
    keywords = extract_keywords(feature['description'])
    if not keywords:
        return []

    with DRIVER.session() as session:
        # Find events in the time window that are either:
        # - Unlinked (no LINKED_TO)
        # - Linked to Session Work pseudo-feature
        # - Linked to a generic/catch-all feature
        # Memgraph duration syntax: add 2 hours buffer after completion
        result = session.run('''
            MATCH (e:Event)
            WHERE e.timestamp >= $created_at
              AND e.timestamp <= $completed_at
            OPTIONAL MATCH (e)-[:LINKED_TO]->(f:Feature)
            WITH e, f
            WHERE f IS NULL
               OR f.is_session_work = true
               OR f.description CONTAINS 'Phoenix LiveView migration'
            RETURN e.id as id,
                   e.event_type as event_type,
                   e.tool_name as tool_name,
                   e.payload as payload,
                   e.summary as summary,
                   toString(e.timestamp) as timestamp,
                   f.id as current_feature_id,
                   f.description as current_feature_desc
            ORDER BY e.timestamp
        ''', {
            'created_at': feature['created_at'],
            'completed_at': feature['completed_at']
        })

        candidates = []
        for r in result:
            event = dict(r)
            # Score the event against feature keywords
            event_text = ' '.join([
                str(event.get('tool_name', '')),
                str(event.get('summary', '')),
                str(event.get('payload', ''))
            ])
            event_keywords = extract_keywords(event_text)

            # Calculate match score
            matches = keywords & event_keywords
            if matches:
                event['match_score'] = len(matches)
                event['matching_keywords'] = list(matches)
                candidates.append(event)

        return sorted(candidates, key=lambda x: -x['match_score'])


def reattribute_event(event_id: str, old_feature_id: str | None, new_feature_id: str, dry_run: bool = True):
    """Remove old LINKED_TO and create new one."""
    if dry_run:
        return

    with DRIVER.session() as session:
        # Remove existing LINKED_TO if any
        if old_feature_id:
            session.run('''
                MATCH (e:Event {id: $eventId})-[r:LINKED_TO]->(f:Feature {id: $oldFeatureId})
                DELETE r
            ''', {'eventId': event_id, 'oldFeatureId': old_feature_id})

        # Create new LINKED_TO
        session.run('''
            MATCH (e:Event {id: $eventId})
            MATCH (f:Feature {id: $newFeatureId})
            MERGE (e)-[:LINKED_TO]->(f)
        ''', {'eventId': event_id, 'newFeatureId': new_feature_id})


def score_event_for_feature(event: dict, feature: dict) -> tuple[int, list[str]]:
    """Score an event against a feature. Returns (score, matching_keywords)."""
    feature_keywords = extract_keywords(feature['description'])
    if not feature_keywords:
        return 0, []

    event_text = ' '.join([
        str(event.get('tool_name', '')),
        str(event.get('summary', '')),
        str(event.get('payload', ''))
    ])
    event_keywords = extract_keywords(event_text)

    matches = feature_keywords & event_keywords
    return len(matches), list(matches)


def get_all_candidate_events(features: list[dict]) -> list[dict]:
    """Get all events in the time window of any feature."""
    if not features:
        return []

    # Find the overall time range
    min_created = min(f['created_at'] for f in features)
    max_completed = max(f['completed_at'] for f in features)

    with DRIVER.session() as session:
        result = session.run('''
            MATCH (e:Event)
            WHERE e.timestamp >= $min_created
              AND e.timestamp <= $max_completed
            OPTIONAL MATCH (e)-[:LINKED_TO]->(f:Feature)
            WITH e, f
            WHERE f IS NULL
               OR f.is_session_work = true
               OR f.description CONTAINS 'Phoenix LiveView migration'
            RETURN e.id as id,
                   e.event_type as event_type,
                   e.tool_name as tool_name,
                   e.payload as payload,
                   e.summary as summary,
                   toString(e.timestamp) as timestamp,
                   e.timestamp as raw_timestamp,
                   f.id as current_feature_id,
                   f.description as current_feature_desc
            ORDER BY e.timestamp
        ''', {
            'min_created': min_created,
            'max_completed': max_completed
        })
        return [dict(r) for r in result]


def main():
    parser = argparse.ArgumentParser(description='Re-attribute events to correct features')
    parser.add_argument('--dry-run', action='store_true', default=True,
                        help='Show what would be changed without making changes (default: True)')
    parser.add_argument('--apply', action='store_true',
                        help='Actually apply the changes')
    parser.add_argument('--min-score', type=int, default=3,
                        help='Minimum keyword match score to reattribute (default: 3)')
    args = parser.parse_args()

    dry_run = not args.apply

    print(f"{'[DRY RUN] ' if dry_run else ''}Event Re-Attribution (Best-Match Algorithm)")
    print("=" * 60)

    features = get_features_without_events()
    print(f"\nFound {len(features)} completed features with 0 events")

    if not features:
        print("Nothing to do.")
        DRIVER.close()
        return

    # Get all candidate events
    print("Fetching candidate events...")
    all_events = get_all_candidate_events(features)
    print(f"Found {len(all_events)} candidate events in time window\n")

    # For each event, find the BEST matching feature
    event_assignments = {}  # event_id -> (feature, score, keywords)

    for event in all_events:
        event_ts = event.get('raw_timestamp')
        best_feature = None
        best_score = 0
        best_keywords = []

        for feature in features:
            # Check if event is within feature's time window
            if event_ts and (event_ts < feature['created_at'] or event_ts > feature['completed_at']):
                continue

            score, keywords = score_event_for_feature(event, feature)
            if score > best_score:
                best_score = score
                best_feature = feature
                best_keywords = keywords

        if best_feature and best_score >= args.min_score:
            event_assignments[event['id']] = {
                'event': event,
                'feature': best_feature,
                'score': best_score,
                'keywords': best_keywords
            }

    # Group by feature for display
    feature_events = {}  # feature_id -> list of events
    for assignment in event_assignments.values():
        fid = assignment['feature']['id']
        if fid not in feature_events:
            feature_events[fid] = []
        feature_events[fid].append(assignment)

    total_reattributed = 0

    for feature in features:
        print(f"\n📦 {feature['description'][:60]}...")
        print(f"   Window: {str(feature['created_at'])[:19]} - {str(feature['completed_at'])[:19]}")

        assignments = feature_events.get(feature['id'], [])
        if not assignments:
            print(f"   ⚪ No matching events found (score >= {args.min_score})")
            continue

        print(f"   ✓ {len(assignments)} events matched")

        for a in sorted(assignments, key=lambda x: -x['score'])[:5]:
            event = a['event']
            print(f"      • [{a['score']}] {event['event_type']}: {event['tool_name'] or 'N/A'}")
            print(f"        Keywords: {', '.join(a['keywords'][:5])}")

            reattribute_event(
                event['id'],
                event.get('current_feature_id'),
                feature['id'],
                dry_run=dry_run
            )
            total_reattributed += 1

        # Reattribute remaining without detailed output
        for a in sorted(assignments, key=lambda x: -x['score'])[5:]:
            reattribute_event(
                a['event']['id'],
                a['event'].get('current_feature_id'),
                feature['id'],
                dry_run=dry_run
            )
            total_reattributed += 1

    print(f"\n{'=' * 60}")
    print(f"{'[DRY RUN] Would reattribute' if dry_run else 'Reattributed'}: {total_reattributed} events")

    if dry_run and total_reattributed > 0:
        print(f"\nTo apply changes, run with: --apply")

    DRIVER.close()


if __name__ == "__main__":
    main()
