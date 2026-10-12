# Command reference (generated from the registry of this build)

Every command the engine registers, with its label and menu path. Generated with `soundcraft-cli commands`; parameters, risk, undo behaviour and known limits are available per command through `help.search` / Help › Command Help, and the active shortcut comes from the chosen profile (Window › Keyboard Shortcuts). UI-layer commands (`window.*`, `ui.*`, `remote.*`, `monitor.*`) are listed by Help › Command Help.

| Command | Label | Menu |
|---|---|---|
| `session.new` | New... | File |
| `session.open` | Open Session... | File |
| `session.close` | Close Session | File |
| `session.save` | Save Session | File |
| `session.save_as` | Save Session As... | File |
| `session.save_copy` | Save Session Copy In... | File |
| `session.save_template` | Save As Template... | File |
| `session.revert` | Revert Session to Saved... | File |
| `file.import_audio` | Audio... | File > Import |
| `file.import_midi` | MIDI... | File > Import |
| `file.import_video` | Video... | File > Import |
| `file.bounce_mix` | Bounce Mix... | File |
| `file.bounce_stems` | Bounce Stems... |  |
| `file.export_midi` | MIDI... | File > Export |
| `file.export_session_text` | Session Info as Text... | File > Export |
| `file.export_clips` | Export Clips as Files... |  |
| `file.export_selected_tracks_as_session` | Selected Tracks as New Session... | File > Export |
| `file.export_sibelius` | Sibelius... | File > Export |
| `file.send_to_sibelius` | Sibelius | File > Send To... |
| `file.print_score` | Print Score... | File |
| `file.score_setup` | Score Setup... | File |
| `file.export_clip_groups` | Export Clip Groups... |  |
| `file.import_clip_groups` | Clip Groups... | File > Import |
| `file.get_info` | Get Info... | File |
| `app.quit` | Quit |  |
| `edit.undo` | Undo | Edit |
| `edit.redo` | Redo | Edit |
| `edit.restore_last_selection` | Restore Last Selection | Edit |
| `edit.cut` | Cut | Edit |
| `edit.copy` | Copy | Edit |
| `edit.paste` | Paste | Edit |
| `edit.clear` | Clear | Edit |
| `edit.cut_clip_gain` | Clip Gain | Edit > Cut Special |
| `edit.clear_clip_gain` | Clip Gain | Edit > Clear Special |
| `edit.cut_all_automation` | All Automation | Edit > Cut Special |
| `edit.clear_all_automation` | All Automation | Edit > Clear Special |
| `edit.clear_pan_automation` | Pan Automation | Edit > Clear Special |
| `edit.clear_plugin_automation` | Plugin Automation | Edit > Clear Special |
| `edit.paste_repeat_to_fill` | Repeat to Fill Selection | Edit > Paste Special |
| `edit.select_all` | Select All | Edit |
| `edit.select` | Set Edit Selection |  |
| `edit.select_none` | Deselect All |  |
| `edit.selection_to_timeline` | Change Timeline to Match Edit | Edit > Selection |
| `edit.timeline_to_selection` | Change Edit to Match Timeline | Edit > Selection |
| `edit.move_selection_left` | Move Edit Left | Edit > Selection |
| `edit.move_selection_right` | Move Edit Right | Edit > Selection |
| `edit.halve_selection` | Halve Edit | Edit > Selection |
| `edit.double_selection` | Double Edit | Edit > Selection |
| `edit.extend_selection_up` | Extend Edit Up | Edit > Selection |
| `edit.extend_selection_down` | Extend Edit Down | Edit > Selection |
| `edit.remove_selection_top` | Remove Edit from Top | Edit > Selection |
| `edit.remove_selection_bottom` | Remove Edit from Bottom | Edit > Selection |
| `edit.duplicate` | Duplicate | Edit |
| `edit.repeat` | Repeat... | Edit |
| `edit.shift` | Shift... | Edit |
| `edit.insert_silence` | Insert Silence | Edit |
| `edit.separate` | At Selection | Edit > Separate |
| `edit.separate_on_grid` | On Grid | Edit > Separate |
| `edit.separate_at_transients` | At Transients | Edit > Separate |
| `edit.heal` | Heal Separation | Edit |
| `edit.trim_to_selection` | To Selection | Edit > Trim |
| `edit.trim_start_to_insertion` | Start to Insertion | Edit > Trim |
| `edit.trim_end_to_insertion` | End to Insertion | Edit > Trim |
| `edit.trim_to_file_boundaries` | To File Boundaries | Edit > Trim |
| `edit.trim_to_fill_selection` | To Fill Selection | Edit > Trim |
| `edit.mute_clips` | Mute | Edit |
| `edit.copy_to_new_playlist` | New Playlist | Edit > Copy Selection to... |
| `edit.strip_silence` | Strip Silence | Edit |
| `edit.fades_create` | Create... | Edit > Fades |
| `edit.fades_delete` | Delete | Edit > Fades |
| `edit.fade_to_start` | Fade to Start | Edit > Fades |
| `edit.fade_to_end` | Fade to End | Edit > Fades |
| `edit.nudge` | Nudge |  |
| `edit.mode` | Edit Mode |  |
| `edit.tool` | Edit Tool |  |
| `edit.move_clips` | Move Clips |  |
| `edit.consolidate` | Consolidate | Edit |
| `edit.cut_pan_automation` | Pan Automation | Edit > Cut Special |
| `edit.cut_plugin_automation` | Plugin Automation | Edit > Cut Special |
| `edit.copy_all_automation` | All Automation | Edit > Copy Special |
| `edit.copy_pan_automation` | Pan Automation | Edit > Copy Special |
| `edit.copy_plugin_automation` | Plugin Automation | Edit > Copy Special |
| `edit.copy_clip_gain` | Clip Gain | Edit > Copy Special |
| `edit.paste_clip_gain` | Paste Clip Gain |  |
| `edit.copy_clip_effects` | Clip Effects | Edit > Copy Special |
| `edit.cut_clip_effects` | Clip Effects | Edit > Cut Special |
| `edit.clear_clip_effects` | Clip Effects | Edit > Clear Special |
| `edit.paste_clip_effects` | Paste Clip Effects |  |
| `edit.paste_merge_midi` | Merge MIDI | Edit > Paste Special |
| `edit.paste_merge_markers` | Merge Markers | Edit > Paste Special |
| `edit.paste_to_current_automation` | To Current Automation Type | Edit > Paste Special |
| `edit.play_timeline` | Play Timeline | Edit > Selection |
| `edit.duplicate_extend` | Duplicate and Extend Edit | Edit > Selection |
| `edit.extend_up_to_members` | Extend Edit Up to Members | Edit > Selection |
| `edit.extend_down_to_members` | Extend Edit Down to Members | Edit > Selection |
| `edit.marker_lane_move_up` | Move Edit Selection To Marker Lane Above Edit > Selection |  |
| `edit.marker_lane_move_down` | Move Edit Selection To Marker Lane Below Edit > Selection |  |
| `edit.marker_lane_extend_up` | Extend Edit Selection To Marker Lane Above Edit > Selection |  |
| `edit.marker_lane_extend_down` | Extend Edit Selection To Marker Lane Below Edit > Selection |  |
| `edit.space_clips` | Space Clips… | Edit |
| `edit.snap_next` | Next | Edit > Snap to |
| `edit.snap_previous` | Previous | Edit > Snap to |
| `edit.copy_to_target_playlist` | Target Playlist | Edit > Copy Selection to... |
| `edit.copy_to_duplicate_playlist` | Duplicate of Main Playlist | Edit > Copy Selection to... |
| `edit.move_to_target_playlist` | Target Playlist | Edit > Move Selection to... |
| `edit.move_to_new_playlist` | New Playlist | Edit > Move Selection to... |
| `edit.move_to_duplicate_playlist` | Duplicate of Main Playlist | Edit > Move Selection to... |
| `edit.tce_to_timeline` | TCE Edit to Timeline Selection | Edit |
| `edit.automation_coalesce_volume_to_clip_gain` | Coalesce Volume Automation to Clip Gain Edit > Automation |  |
| `edit.automation_coalesce_clip_gain_to_volume` | Coalesce Clip Gain to Volume Automation Edit > Automation |  |
| `edit.automation_trim_to_all` | Trim to All Enabled | Edit > Automation |
| `edit.automation_glide_to_all` | Glide to All Enabled | Edit > Automation |
| `edit.automation_glide_pan` | Glide Pan Only | Edit > Automation |
| `view.zoom_in` | Zoom In |  |
| `view.zoom_out` | Zoom Out |  |
| `view.zoom_set` | Set Zoom |  |
| `view.zoom_preset` | Zoom Preset |  |
| `view.zoom_to_selection` | Zoom to Selection |  |
| `view.zoom_fit` | Fill Window With Session |  |
| `view.scroll` | Scroll Timeline |  |
| `view.waveform_zoom` | Waveform Zoom |  |
| `view.ruler` | Rulers | View > Rulers |
| `view.main_counter` | Main Counter | View > Main Counter |
| `view.sub_counter` | Sub Counter |  |
| `view.grid` | Grid Value |  |
| `view.grid_lines` | Show Grid Lines |  |
| `view.nudge` | Nudge Value |  |
| `track.new` | New... | Track |
| `track.group` | Group... | Track |
| `track.ungroup` | Delete Group |  |
| `track.group_toggle` | Toggle Group Active |  |
| `track.duplicate` | Duplicate... | Track |
| `track.split_into_mono` | Split into Mono | Track |
| `track.make_inactive` | Make Inactive | Track |
| `track.move_to_new_folder` | Move to New Folder... | Track |
| `track.change_width` | Change Track Width | Track > Change Track Width |
| `track.delete` | Delete | Track |
| `track.rename` | Rename Track |  |
| `track.comments` | Track Comments |  |
| `track.color` | Track Color |  |
| `track.height` | Track Height |  |
| `track.view` | Track View |  |
| `track.hide` | Hide Track |  |
| `track.pin` | Pin Track |  |
| `track.move` | Move Track |  |
| `track.freeze` | Freeze | Track |
| `track.commit` | Commit... | Track |
| `track.bounce` | Bounce... | Track |
| `track.bypass_inserts` | All | Track > Bypass Inserts |
| `track.mute_sends` | All | Track > Mute Sends |
| `track.set_record_to_input_only` | Set Record Tracks to Input Only | Track |
| `track.scroll_to` | Scroll to Track... | Track |
| `track.clear_clip_indicators` | Clear All Clip Indicators | Track |
| `track.clear_trim_automation` | Clear Trim Automation | Track |
| `track.create_click` | Create Click Track | Track |
| `track.playlist_new` | New Playlist |  |
| `track.playlist_duplicate` | Duplicate Playlist |  |
| `track.playlist_select` | Select Playlist |  |
| `track.playlist_delete_unused` | Delete Unused Playlists |  |
| `track.folder_toggle` | Open/Close Folder |  |
| `track.vca` | Assign to VCA |  |
| `track.playlist_promote` | Promote to Main Playlist |  |
| `track.input` | Track Input |  |
| `track.output` | Track Output |  |
| `track.timebase` | Track Timebase |  |
| `track.elastic` | Elastic Audio |  |
| `clip.edit_lock` | Edit Lock/Unlock | Clip |
| `clip.time_lock` | Time Lock/Unlock | Clip |
| `clip.send_to_back` | Send to Back | Clip |
| `clip.bring_to_front` | Bring to Front | Clip |
| `clip.rating` | Rating | Clip > Rating |
| `clip.group` | Group | Clip |
| `clip.ungroup` | Ungroup | Clip |
| `clip.ungroup_all` | Ungroup All | Clip |
| `clip.loop` | Loop... | Clip |
| `clip.rename` | Rename... | Clip |
| `clip.gain` | Clip Gain |  |
| `clip.gain_nudge_up` | Nudge Clip Gain Up |  |
| `clip.gain_nudge_down` | Nudge Clip Gain Down |  |
| `clip.gain_render` | Render | Clip > Clip Gain |
| `clip.gain_bypass` | Bypass | Clip > Clip Gain |
| `clip.identify_sync_point` | Identify Sync Point | Clip |
| `clip.quantize_to_grid` | Quantize to Grid | Clip |
| `clip.color` | Clip Color |  |
| `clip.elastic_properties` | Elastic Properties | Clip |
| `clip.remove_warp` | Remove Warp | Clip |
| `clip.place_source` | Place Audio File |  |
| `clip.capture` | Capture... | Clip |
| `event.tempo` | Tempo Change |  |
| `event.tempo_remove` | Remove Tempo Change |  |
| `event.meter` | Change Meter... | Event > Time Operations |
| `event.tempo_scale` | Scale... | Event > Tempo Operations |
| `event.tempo_constant` | Constant... | Event > Tempo Operations |
| `event.insert_time` | Insert Time... | Event > Time Operations |
| `event.cut_time` | Cut Time... | Event > Time Operations |
| `event.quantize` | Quantize... | Event > MIDI Operations |
| `event.transpose` | Transpose... | Event > MIDI Operations |
| `event.change_velocity` | Change Velocity... | Event > MIDI Operations |
| `event.change_duration` | Change Duration... | Event > MIDI Operations |
| `event.remove_duplicate_notes` | Remove Duplicate Notes | Event |
| `event.add_key_change` | Add Key Change... | Event |
| `event.renumber_bars` | Renumber Bars... | Event |
| `event.all_notes_off` | All MIDI Notes Off | Event |
| `event.midi_note` | Add MIDI Note |  |
| `markers.add` | New Memory Location |  |
| `markers.delete` | Delete Memory Location |  |
| `markers.edit` | Edit Memory Location |  |
| `markers.recall` | Recall Memory Location |  |
| `markers.next` | Go to Next Marker |  |
| `markers.previous` | Go to Previous Marker |  |
| `mix.volume` | Set Volume |  |
| `mix.pan` | Set Pan |  |
| `mix.mute` | Mute |  |
| `mix.solo` | Solo |  |
| `mix.solo_safe` | Solo Safe |  |
| `mix.clear_solos` | Clear All Solos |  |
| `mix.clear_mutes` | Clear All Mutes |  |
| `mix.record_arm` | Record Enable |  |
| `mix.input_monitor` | Input Monitoring |  |
| `mix.phase_invert` | Phase Invert |  |
| `mix.trim` | Input Trim |  |
| `mix.automation_mode` | Automation Mode |  |
| `mix.insert` | Insert Plugin |  |
| `mix.insert_remove` | Remove Insert |  |
| `mix.insert_bypass` | Bypass Insert |  |
| `mix.insert_param` | Set Plugin Parameter |  |
| `mix.insert_sidechain` | Insert Sidechain Source |  |
| `mix.insert_params` | Set Plugin Parameters |  |
| `mix.insert_post` | Insert Pre/Post Fader |  |
| `mix.insert_move` | Move Insert |  |
| `mix.send` | Assign Send |  |
| `mix.send_remove` | Remove Send |  |
| `mix.send_level` | Send Level |  |
| `mix.new_bus` | New Bus |  |
| `automation.set_point` | Add Automation Breakpoint |  |
| `automation.write_range` | Write Automation |  |
| `automation.clear` | Clear Automation |  |
| `automation.thin` | Thin | Edit > Automation |
| `automation.thin_all` | Thin All | Edit > Automation |
| `automation.write_to_current` | Write to Current | Edit > Automation |
| `automation.write_to_all` | Write to All Enabled | Edit > Automation |
| `automation.volume_to_clip_gain` | Convert Volume Automation to Clip Gain Edit > Automation |  |
| `automation.clip_gain_to_volume` | Convert Clip Gain to Volume Automation Edit > Automation |  |
| `automation.copy_to_send` | Copy to Send... | Edit > Automation |
| `options.loop_record` | Loop Record | Options |
| `options.quickpunch` | QuickPunch | Options |
| `options.pre_post_roll` | Pre/Post-Roll | Options |
| `options.loop_playback` | Loop Playback | Options |
| `options.link_timeline_edit` | Link Timeline and Edit Selection | Options |
| `options.link_track_edit` | Link Track and Edit Selection | Options |
| `options.insertion_follows_playback` | Insertion Follows Playback | Options |
| `options.tab_to_transient` | Tab to Transient | Options |
| `options.mirrored_midi` | Mirrored MIDI Editing | Options |
| `options.automation_follows_edit` | Automation Follows Edit | Options |
| `options.markers_follow_edit` | Markers Follow Edit | Options |
| `options.layered_editing` | Layered Editing | Options |
| `options.click` | Click | Options |
| `options.countoff` | Count Off |  |
| `options.midi_merge` | MIDI Merge |  |
| `options.pre_fader_metering` | Pre-Fader Metering | Options |
| `options.delay_compensation` | Delay Compensation | Options |
| `options.destructive_record` | Destructive Record | Options |
| `options.trackpunch` | TrackPunch | Options |
| `options.scrolling` | Edit Window Scrolling | Options > Edit Window Scrolling |
| `options.solo_mode` | Solo Mode | Options > Solo Mode |
| `options.keyboard_focus` | Keyboard Focus |  |
| `options.pre_roll` | Pre-Roll Amount |  |
| `options.post_roll` | Post-Roll Amount |  |
| `setup.session` | Session | Setup |
| `setup.preferences` | Preferences... | Setup |
| `setup.reset_edit_state` | Reset Edit Settings |  |
| `transport.play` | Play |  |
| `transport.stop` | Stop |  |
| `transport.toggle` | Play/Stop |  |
| `transport.record` | Record |  |
| `transport.pause` | Pause |  |
| `transport.half_speed` | Half-Speed Playback |  |
| `transport.play_selection` | Play Edit | Edit > Selection |
| `transport.rtz` | Return to Zero |  |
| `transport.go_to_end` | Go to End |  |
| `transport.rewind` | Rewind |  |
| `transport.fast_forward` | Fast Forward |  |
| `transport.locate` | Locate |  |
| `transport.status` | Transport Status |  |
| `session.inspect` | Inspect Session |  |
| `track.inspect` | Inspect Track |  |
| `clip.inspect` | Inspect Clip |  |
| `engine.commands` | List Commands |  |
| `engine.history` | Undo History | Window |
| `engine.plugins` | List Plugins |  |
| `session.missing_plugins` | Missing Plugins |  |
| `engine.parity` | Parity Report |  |
| `session.format_time` | Format Time |  |
| `session.audio_health` | Session Audio Health |  |
| `audiosuite.process` | AudioSuite Process |  |
| `edit.copy_audio_as_midi` | Audio as MIDI | Edit > Copy Special |
| `midi.notes` | List MIDI Notes |  |
| `midi.new_clip` | New MIDI Clip |  |
| `midi.note_add` | Add Note |  |
| `midi.note_edit` | Edit Note |  |
| `midi.note_delete` | Delete Notes |  |
| `midi.notes_move` | Move Notes |  |
| `view.edit_comments` | Comments | View > Edit Window Views |
| `view.edit_mic_preamps` | Mic Preamps | View > Edit Window Views |
| `view.edit_instruments` | Instruments | View > Edit Window Views |
| `view.edit_inserts_ae` | Inserts A-E | View > Edit Window Views |
| `view.edit_inserts_fj` | Inserts F-J | View > Edit Window Views |
| `view.edit_sends_ae` | Sends A-E | View > Edit Window Views |
| `view.edit_sends_fj` | Sends F-J | View > Edit Window Views |
| `view.edit_io` | I/O | View > Edit Window Views |
| `view.edit_object` | Object | View > Edit Window Views |
| `view.edit_realtime_properties` | Real-Time Properties | View > Edit Window Views |
| `view.edit_track_color` | Track Color | View > Edit Window Views |
| `view.edit_marker_controls` | Marker Controls | View > Edit Window Views |
| `view.edit_pinned_tracks` | Pinned Tracks | View > Edit Window Views |
| `view.edit_all` | All | View > Edit Window Views |
| `view.edit_minimal` | Minimal | View > Edit Window Views |
| `view.ruler_timecode2` | Timecode 2 | View > Rulers |
| `view.ruler_markers2` | Markers 2 | View > Rulers |
| `view.ruler_markers3` | Markers 3 | View > Rulers |
| `view.ruler_markers4` | Markers 4 | View > Rulers |
| `view.ruler_markers5` | Markers 5 | View > Rulers |
| `view.ruler_all_markers` | All Marker Rulers | View > Rulers |
| `view.ruler_all` | All | View > Rulers |
| `view.ruler_minimal` | Minimal | View > Rulers |
| `view.ruler_tempo_editor` | Tempo Editor | View > Rulers > Tempo |
| `view.ruler_key_staff` | Key Signature Staff | View > Rulers > Key Signature |
| `view.marker_track_lane` | Track Lane | View > Marker Displays |
| `view.marker_ruler_lines` | Ruler Lines | View > Marker Displays |
| `view.marker_folder_lines` | Folder Lines | View > Marker Displays |
| `view.marker_ruler_lane_color` | Ruler Lane Color | View > Marker Displays |
| `view.marker_track_lane_color` | Track Lane Color | View > Marker Displays |
| `view.midi_input_display` | MIDI Input Display | View > Other Displays |
| `view.midi_editor_clip_effects` | Clip Effects | View > Other Displays > Lower Dock > MIDI Editor |
| `view.clip_sync_point` | Sync Point | View > Clip |
| `view.clip_processing_state` | Processing State | View > Clip |
| `view.clip_name` | Name | View > Clip |
| `view.clip_channel_name` | Channel Name | View > Clip |
| `view.clip_scene_take` | Scene And Take | View > Clip |
| `view.clip_rating` | Rating | View > Clip |
| `view.clip_overlap_shadows` | Overlap Shadows | View > Clip |
| `view.clip_transparency` | Transparency | View > Clip |
| `view.clip_overwrite_indicator` | Overwrite Indicator | View > Clip |
| `view.clip_copy_move_indicators` | Copy/Move Indicators | View > Clip |
| `view.clip_gain_line` | Clip Gain Line | View > Clip |
| `view.clip_gain_info` | Clip Gain Info | View > Clip |
| `view.clip_effects_status` | Clip Effects Status | View > Clip |
| `view.clip_ara_note_overlay` | ARA Note Overlay | View > Clip |
| `view.clip_time_current` | Current Time | View > Clip |
| `view.clip_time_original` | Original Time Stamp | View > Clip |
| `view.clip_time_user` | User Time Stamp | View > Clip |
| `view.clip_time_none` | No Time | View > Clip |
| `view.clip_all_channels` | Display on All Channels | View > Clip |
| `view.waveform_peak` | Peak | View > Waveforms |
| `view.waveform_power` | Power | View > Waveforms |
| `view.waveform_rectified` | Rectified | View > Waveforms |
| `view.waveform_outlines` | Outlines | View > Waveforms |
| `view.waveform_overlapped_crossfades` | Overlapped Crossfades | View > Waveforms |
| `view.automation_trim_playlist` | Trim Playlist | View > Automation |
| `view.automation_composite_playlist` | Composite Playlist | View > Automation |
| `view.expanded_sends_all` | All | View > Expanded Sends |
| `view.expanded_sends_none` | None | View > Expanded Sends |
| `view.expanded_send_a` | Send A | View > Expanded Sends |
| `view.expanded_send_b` | Send B | View > Expanded Sends |
| `view.expanded_send_c` | Send C | View > Expanded Sends |
| `view.expanded_send_d` | Send D | View > Expanded Sends |
| `view.expanded_send_e` | Send E | View > Expanded Sends |
| `view.expanded_send_f` | Send F | View > Expanded Sends |
| `view.expanded_send_g` | Send G | View > Expanded Sends |
| `view.expanded_send_h` | Send H | View > Expanded Sends |
| `view.expanded_send_i` | Send I | View > Expanded Sends |
| `view.expanded_send_j` | Send J | View > Expanded Sends |
| `view.track_number` | Track Number | View |
| `view.track_transcription_lane` | Track Transcription Lane | View |
| `view.dropped_video_frame_indicator` | Dropped Video Frame Indicator | View |
| `view.transport_counters` | Counters | View > Transport |
| `view.transport_midi_controls` | MIDI Controls | View > Transport |
| `view.transport_synchronization` | Synchronization | View > Transport |
| `view.transport_ableton_link` | Ableton Link | View > Transport |
| `view.transport_output_meters` | Output Meters | View > Transport |
| `view.transport_expanded` | Expanded | View > Transport |
| `view.flags` | View Flags |  |
| `track.object` | Object / Bed |  |
| `track.convert_aux_to_folder` | Convert Aux to Routing Folder | Track |
| `track.extract_midi` | Extract MIDI to New Track | Track |
| `track.save_preset` | Save Track Preset... | Track |
| `track.load_preset` | Load Track Preset |  |
| `track.bypass_inserts_ae` | Inserts A-E | Track > Bypass Inserts |
| `track.bypass_inserts_fj` | Inserts F-J | Track > Bypass Inserts |
| `track.bypass_eq` | EQ | Track > Bypass Inserts |
| `track.bypass_dynamics` | Dynamics | Track > Bypass Inserts |
| `track.bypass_reverb` | Reverb | Track > Bypass Inserts |
| `track.bypass_delay` | Delay | Track > Bypass Inserts |
| `track.bypass_modulation` | Modulation | Track > Bypass Inserts |
| `track.mute_sends_ae` | Sends A-E | Track > Mute Sends |
| `track.mute_sends_fj` | Sends F-J | Track > Mute Sends |
| `track.write_midi_rtp` | Write MIDI Real-Time Properties | Track |
| `track.trim_automation_write` | Write Trim Automation |  |
| `track.coalesce_trim` | Coalesce Trim Automation | Track |
| `track.coalesce_vca` | Coalesce VCA Controller Automation Track |  |
| `track.designate_target_playlist` | Designate as Target Playlist | Track |
| `track.show_target_playlist` | Show Target Playlist | Track |
| `track.toggle_recent_playlist` | Toggle Recent Playlist | Track |
| `clip.regroup` | Regroup | Clip |
| `clip.unloop` | Unloop... | Clip |
| `clip.effects_set` | Set Clip Effects |  |
| `clip.effects_bypass` | Bypass | Clip > Clip Effects |
| `clip.effects_render` | Render | Clip > Clip Effects |
| `clip.conform_to_tempo` | Conform to Tempo | Clip |
| `clip.remove_pitch_shift` | Remove Pitch Shift | Clip |
| `event.time_ops_window` | Time Operations Window | Event > Time Operations |
| `event.tempo_ops_window` | Tempo Operations Window | Event > Tempo Operations |
| `event.midi_ops_window` | MIDI Operations Window | Event > MIDI Operations |
| `event.move_song_start` | Move Song Start... | Event > Time Operations |
| `event.tempo_linear` | Linear... | Event > Tempo Operations |
| `event.tempo_parabolic` | Parabolic... | Event > Tempo Operations |
| `event.tempo_s_curve` | S-Curve... | Event > Tempo Operations |
| `event.tempo_stretch` | Stretch... | Event > Tempo Operations |
| `event.select_split_notes` | Select/Split Notes... | Event > MIDI Operations |
| `event.input_quantize` | Input Quantize... | Event > MIDI Operations |
| `event.step_input` | Step Input... | Event > MIDI Operations |
| `event.restore_performance` | Restore Performance... | Event > MIDI Operations |
| `event.flatten_performance` | Flatten Performance... | Event > MIDI Operations |
| `event.midi_track_offsets` | MIDI Track Offsets | Event |
| `event.midi_rtp` | MIDI Real-Time Properties | Event |
| `event.extract_chords` | Extract Chords from Selection | Event |
| `event.beat_detective` | Beat Detective | Event |
| `event.identify_beat` | Identify Beat... | Event |
| `event.retrospective_record` | Retrospective Record | Event |
| `options.destructive_punch` | DestructivePunch | Options |
| `options.transport_online` | Transport Online | Options |
| `options.dynamic_transport` | Dynamic Transport | Options |
| `options.marker_target_follows_selection` | Marker Target Follows First Selected Track Options |  |
| `options.midi_thru` | MIDI Thru | Options |
| `options.auto_spot_clips` | Auto-Spot Clips | Options |
| `options.keyboard_lock` | Edit/Tool Mode Keyboard Lock | Options |
| `options.calibration_mode` | Calibration Mode | Options |
| `options.midi_live_mode` | MIDI Live Mode | Options |
| `options.low_latency_monitoring` | Low Latency Monitoring | Options |
| `options.prepare_dpe_tracks` | Prepare DPE Tracks | Options |
| `setup.hardware` | Hardware... | Setup |
| `setup.playback_engine` | Playback Engine... | Setup |
| `setup.disk_allocation` | Disk Allocation... | Setup |
| `setup.peripherals` | Peripherals... | Setup |
| `setup.io` | I/O... | Setup |
| `setup.current_timecode` | Current Timecode Position... | Setup |
| `setup.current_feet_frames` | Current Feet+Frames Position... | Setup |
| `setup.external_timecode_offset` | External Timecode Offset... | Setup |
| `setup.video_sync_offset` | Video Sync Offset... | Setup |
| `setup.click_countoff` | Click/Countoff... | Setup |
| `setup.keyboard_shortcuts` | Keyboard Shortcuts... | Setup |
| `setup.midi_studio` | MIDI Studio... | Setup > MIDI |
| `setup.midi_beat_clock` | MIDI Beat Clock... | Setup > MIDI |
| `setup.midi_input_filter` | MIDI Input Filter... | Setup > MIDI |
| `setup.midi_input_devices` | MIDI Input Devices... | Setup > MIDI |
| `setup.transcription_settings` | Transcription Settings... | Setup |
| `file.import_session_data` | Session Data... | File > Import |
| `engine.clap_plugins` | List CLAP Plugins |  |
| `engine.vst3_plugins` | List VST3 Plugins |  |
| `mix.surround_pan` | Set Surround Pan |  |
| `mix.send_surround_pan` | Set Send Surround Pan |  |
| `setup.main_format` | Main Output Format |  |
| `mix.insert_state` | Store Plugin State |  |
| `mix.instrument` | Set Instrument |  |
| `video.online` | Video Track Online | Options |
| `video.offset` | Set Video Start |  |
| `video.list` | Movies |  |
| `nudge.set` | Nudge Settings |  |
| `nudge.info` | Nudge Info |  |
| `nudge.apply` | Nudge |  |
| `nudge.clips` | Nudge Clips | Edit > Nudge |
| `nudge.contents` | Nudge Clip Contents | Edit > Nudge |
| `nudge.trim_start` | Nudge Trim Start | Edit > Nudge |
| `nudge.trim_end` | Nudge Trim End | Edit > Nudge |
| `nudge.selection` | Nudge Selection Only | Edit > Nudge |
| `nudge.selection_start` | Nudge Selection Start | Edit > Nudge |
| `nudge.selection_end` | Nudge Selection End | Edit > Nudge |
| `bus.create_from_selection` | Create Bus from Selection | Mix |
| `bus.create_fx_return` | Create FX Return | Mix |
| `bus.delete` | Delete Bus |  |
| `bus.validate` | Validate Routing Graph |  |
| `bus.sources` | Bus Sources |  |
| `bus.spill` | Spill Bus |  |
| `mix.view` | Mixer View |  |
| `mix.width` | Stereo Width |  |
| `snapshot.save` | Save Mix Snapshot | Mix |
| `snapshot.recall` | Recall Mix Snapshot | Mix |
| `snapshot.info` | Mix Snapshots |  |
| `mix.slot` | Set Dedicated Slot |  |
| `mix.profile` | Mix Profile | Mix |
| `mix.graph_migrate` | Migrate Mixer Graph |  |
| `mix.slot_state` | Store Slot Plugin State |  |
| `mix.audition_bypass` | Audition Bypass |  |
| `mix.monitor_mode` | Monitoring Mode |  |
| `mix.tracking_deactivate` | Tracking: Deactivate Latent Processors |  |
| `mix.tracking_restore` | Tracking: Restore Processors |  |
| `mix.latency_report` | Latency Report |  |
| `mix.signal_flow` | Signal Flow |  |
| `file.export_path` | Export Named Path |  |
| `automation.draw` | Draw Automation Shape |  |
| `edit.split_at` | Split at Pointer |  |
| `edit.erase_clips` | Erase Clips |  |
| `clip.gain_draw` | Draw Clip Gain |  |
| `edit.tool_link` | Link Arrow/Range |  |
| `edit.alt_tool` | Alternate Tool |  |
| `edit.paint_shape` | Paint Shape |  |
| `audition.start` | Listen (audition) |  |
| `audition.stop` | Stop Listen |  |
| `audition.status` | Listen Status |  |
| `clip.warp_set` | Set Warp Anchors | Clip |
| `clip.warp_clear` | Clear Warp Anchors | Clip |
| `clip.warp_poll` | Apply Finished Bend Renders |  |
| `clip.warp_status` | Bend Status |  |
| `help.search` | Command Help | Help |
| `help.limits` | Known Limits | Help |
| `engine.host_faults` | Plugin Faults |  |
| `session.plugin_versions` | Plugin Versions |  |
| `engine.plugin_capabilities` | Plugin Capabilities |  |
| `assistant.ask` | Ask the Assistant | Help |
| `assistant.execute` | Execute Assistant Plan |  |
| `assistant.provider` | Assistant Provider |  |
| `assistant.transcribe` | Transcribe Speech |  |
| `assistant.speak` | Speak Reply |  |
| `mastering.program` | Mastering Program Source |  |
| `mastering.reference` | Reference Track |  |
| `mastering.level_match` | Level-Match Reference |  |
| `mastering.meters` | Mastering Meters |  |
| `mastering.status` | Mastering Status |  |

528 commands.
