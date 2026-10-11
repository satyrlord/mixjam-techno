"""Read a saved .mixjam song (format version 8) for the verification scripts.

The result uses the field names of the earlier JSON project format, which the
application still opens. The scripts compare songs in that form and write it
back as input. The file's application version and timestamps are left out, so
two saves of one song compare equal.
"""
import json
from pathlib import Path


def _reference(reference):
    if reference['type'] == 'native':
        return {'type': 'native', 'product': reference['product'],
                'native_id': reference['nativeId']}
    return reference


def read_song(path):
    document = json.loads(Path(path).read_text(encoding='utf-8'))
    if document['formatVersion'] != 8:
        raise RuntimeError(f'{path} is not a format version 8 .mixjam song.')
    song = document['song']
    clips = [{'id': item['id'], 'sample_id': item['sampleId'], 'track': track,
              'start_tick': item['startTick'], 'length_ticks': item['durationTicks'],
              'offset_frames': item['offsetFrames'], 'gain': item['gain']}
             for track, lane in enumerate(document['lanes']) for item in lane['placements']]
    imported = []
    for item in document['samples']:
        sample = {'id': item['id'], 'name': item['name'], 'group_name': item['groupName'],
                  'path': item['path'], 'source_bpm': item['sourceBpm']}
        if item['sourceSampleRate'] is not None:
            sample['source_sample_rate'] = item['sourceSampleRate']
        sample['origin'] = item['origin']
        imported.append(sample)
    return {
        'schema_version': 3,
        'title': song['title'],
        'author': song['author'],
        'bpm': song['bpm'],
        'master_gain': song['masterGain'],
        'pitch_ratio': 1.0,
        'tracks': [{'gain': lane['gain'], 'pan': lane['pan'],
                    'left_enabled': lane['leftEnabled'],
                    'right_enabled': lane['rightEnabled']} for lane in document['lanes']],
        'clips': sorted(clips, key=lambda clip: clip['id']),
        'loop_region': song['loopRegion'],
        'imported_samples': imported,
        'missing_samples': [
            {'id': item['id'], 'name': item['name'], 'source_bpm': item['sourceBpm'],
             'sample_rate': item['sampleRate'], 'nominal_ticks': item['nominalTicks'],
             'length_inferred': item['lengthInferred'],
             'reference': _reference(item['reference']), 'reason': item['reason'],
             'metadata': item['metadata']} for item in document['missingSamples']],
        'sample_conflicts': [
            {'reference': item['reference'], 'sample_ids': item['sampleIds'],
             'candidates': item['candidates'], 'selected': item['selected'],
             'confirmed': item['confirmed'], 'source_spans': item['sourceSpans']}
            for item in document['sampleConflicts']],
        'import_notices': [{'source_bpm': item['sourceBpm'], 'target_bpm': item['targetBpm']}
                           for item in document['importNotices']],
        'source_notices': document['sourceNotices'],
        'libraries': document['libraries'],
    }
