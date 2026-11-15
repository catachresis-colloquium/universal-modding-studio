// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//
// Lossless conversion between active fields in the bounded C LevelData and the
// complete snake_case JSON wire model owned by the Idris2 ABI.

const std = @import("std");
const types = @import("types");
const admission = @import("json_admission");

pub const CodecError = error{
    InvalidCount,
    InvalidEnum,
    InvalidIp,
    MissingString,
    MissingMission,
    MissingPhysical,
    MissingObjectives,
};

/// Internal ownership wrapper. The public pointer still addresses an unchanged
/// `LevelData`; its strings and variable-length mission data live in `arena`.
pub const OwnedLevel = struct {
    arena: std.heap.ArenaAllocator,
    level: types.LevelData,
};

pub fn createOwned(backing: std.mem.Allocator) !*OwnedLevel {
    const owned = try backing.create(OwnedLevel);
    owned.* = .{
        .arena = std.heap.ArenaAllocator.init(backing),
        .level = std.mem.zeroes(types.LevelData),
    };
    return owned;
}

pub fn destroyOwned(backing: std.mem.Allocator, level: *types.LevelData) void {
    const owned: *OwnedLevel = @fieldParentPtr("level", level);
    owned.arena.deinit();
    backing.destroy(owned);
}

fn requireString(value: ?[*:0]const u8) CodecError![]const u8 {
    return if (value) |ptr| std.mem.span(ptr) else error.MissingString;
}

fn dupeZ(allocator: std.mem.Allocator, value: []const u8) ![*:0]const u8 {
    return (try allocator.dupeZ(u8, value)).ptr;
}

fn deviceKindName(value: types.DeviceKind) []const u8 {
    return switch (value) {
        .laptop => "laptop",
        .desktop => "desktop",
        .server => "server",
        .router => "router",
        .switch_ => "switch",
        .firewall => "firewall",
        .camera => "camera",
        .access_point => "access_point",
        .patch_panel => "patch_panel",
        .power_supply => "power_supply",
        .phone_system => "phone_system",
        .fibre_hub => "fibre_hub",
    };
}

fn securityName(value: types.SecurityLevel) []const u8 {
    return switch (value) {
        .open => "open",
        .weak => "weak",
        .medium => "medium",
        .strong => "strong",
    };
}

fn guardRankName(value: types.GuardRank) []const u8 {
    return switch (value) {
        .basic_guard => "basic",
        .enforcer => "enforcer",
        .anti_hacker => "anti_hacker",
        .sentinel => "sentinel",
        .assassin => "assassin",
        .elite_guard => "elite",
        .security_chief => "security_chief",
        .rival_hacker => "rival_hacker",
    };
}

fn dogBreedName(value: types.DogBreed) []const u8 {
    return switch (value) {
        .patrol => "patrol",
        .bloodhound => "bloodhound",
        .robo_dog => "robo_dog",
    };
}

fn droneArchetypeName(value: types.DroneArchetype) []const u8 {
    return switch (value) {
        .helper => "helper",
        .hunter => "hunter",
        .killer => "killer",
    };
}

fn conditionName(value: types.ItemCondition) []const u8 {
    return switch (value) {
        .pristine => "pristine",
        .good => "good",
        .worn => "worn",
        .damaged => "damaged",
        .broken => "broken",
    };
}

fn wiringName(value: types.WiringType) []const u8 {
    return switch (value) {
        .patch_panel => "patch_panel",
        .switch_backplane => "switch_backplane",
        .server_rack => "server_rack",
        .fibre_splicing => "fibre_splicing",
        .pbx_comms => "pbx_comms",
    };
}

fn enumPayloadName(comptime T: type, raw: u8) CodecError![]const u8 {
    const value = std.meta.intToEnum(T, raw) catch return error.InvalidEnum;
    return switch (T) {
        types.CableType => switch (value) {
            .ethernet => "ethernet",
            .fibre_lc => "fibre_lc",
            .fibre_sc => "fibre_sc",
            .serial => "serial",
            .usb => "usb",
            .universal => "universal",
        },
        types.AdapterType => switch (value) {
            .ethernet_to_fibre => "ethernet_to_fibre",
            .usb_to_serial => "usb_to_serial",
            .media_converter => "media_converter",
        },
        types.ToolType => switch (value) {
            .crimper => "crimper",
            .splicer => "splicer",
            .multimeter => "multimeter",
            .wire_cutter => "wire_cutter",
            .debugger => "debugger",
        },
        types.ModuleType => switch (value) {
            .sfp => "sfp",
            .gbic => "gbic",
            .qsfp => "qsfp",
            .transceiver => "transceiver",
        },
        types.ConsumableType => switch (value) {
            .battery_pack => "battery_pack",
            .emp => "emp",
            .smoke_grenade => "smoke_grenade",
            .decryptor => "decryptor",
        },
        else => @compileError("unsupported item payload enum"),
    };
}

fn ipText(allocator: std.mem.Allocator, ip: types.IpAddress) ![]const u8 {
    return std.fmt.allocPrint(allocator, "{d}.{d}.{d}.{d}", .{ ip.octet1, ip.octet2, ip.octet3, ip.octet4 });
}

fn itemKindToWire(kind: types.ItemKind) !admission.WireItemKind {
    return switch (kind.tag) {
        .cable => .{ .type = "cable", .sub_type = try enumPayloadName(types.CableType, kind.sub_type) },
        .adapter => .{ .type = "adapter", .sub_type = try enumPayloadName(types.AdapterType, kind.sub_type) },
        .tool => .{ .type = "tool", .sub_type = try enumPayloadName(types.ToolType, kind.sub_type) },
        .module_ => .{ .type = "module", .sub_type = try enumPayloadName(types.ModuleType, kind.sub_type) },
        .storage => .{ .type = "storage", .capacity = kind.capacity },
        .consumable => .{ .type = "consumable", .sub_type = try enumPayloadName(types.ConsumableType, kind.sub_type) },
        .keycard => .{ .type = "keycard", .zone = try requireString(kind.zone_name) },
        .radio => .{ .type = "radio" },
    };
}

fn whitelistToWire(allocator: std.mem.Allocator, value: ?[*:null]const ?[*:0]const u8) !?[]const []const u8 {
    const ptr = value orelse return null;
    const entries = std.mem.span(ptr);
    const result = try allocator.alloc([]const u8, entries.len);
    for (entries, 0..) |entry, index| result[index] = try requireString(entry);
    return result;
}

fn checkCounts(level: *const types.LevelData) CodecError!void {
    if (level.devices_len > types.MAX_DEVICES or level.zones_len > types.MAX_ZONES or
        level.guards_len > types.MAX_GUARDS or level.dogs_len > types.MAX_DOGS or
        level.drones_len > types.MAX_DRONES or level.assassins_len > types.MAX_ASSASSINS or
        level.items_len > types.MAX_ITEMS or level.wiring_len > types.MAX_WIRING or
        level.zone_transitions_len > types.MAX_ZONE_TRANSITIONS or
        level.device_defences_len > types.MAX_DEVICE_DEFENCES or
        level.mission.objectives_len > types.MAX_OBJECTIVES)
        return error.InvalidCount;
    if (level.mission.objectives_len > 0 and level.mission.objectives == null)
        return error.MissingObjectives;
}

/// Serialize every active bounded C field using the textual Idris2 wire vocabulary.
pub fn serializeLevelJson(allocator: std.mem.Allocator, level: *const types.LevelData, writer: anytype) !void {
    try checkCounts(level);
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    const temp = arena.allocator();

    const devices = try temp.alloc(admission.WireDevice, level.devices_len);
    for (level.devices[0..level.devices_len], 0..) |device, index| devices[index] = .{
        .kind = deviceKindName(device.kind),
        .ip = try ipText(temp, device.ip),
        .name = try requireString(device.name),
        .security = securityName(device.security),
    };

    const zones = try temp.alloc(admission.WireZone, level.zones_len);
    for (level.zones[0..level.zones_len], 0..) |zone, index| zones[index] = .{
        .name = try requireString(zone.name),
        .security_tier = zone.security_tier,
    };

    const guards = try temp.alloc(admission.WireGuard, level.guards_len);
    for (level.guards[0..level.guards_len], 0..) |guard, index| guards[index] = .{
        .world_x = guard.world_x.position,
        .zone = try requireString(guard.zone),
        .rank = guardRankName(guard.rank),
        .patrol_radius = guard.patrol_radius,
    };

    const dogs = try temp.alloc(admission.WireDog, level.dogs_len);
    for (level.dogs[0..level.dogs_len], 0..) |dog, index| dogs[index] = .{
        .world_x = dog.world_x.position,
        .breed = dogBreedName(dog.breed),
        .patrol_radius = dog.patrol_radius,
    };

    const drones = try temp.alloc(admission.WireDrone, level.drones_len);
    for (level.drones[0..level.drones_len], 0..) |drone, index| drones[index] = .{
        .world_x = drone.world_x.position,
        .archetype = droneArchetypeName(drone.archetype),
        .altitude = drone.altitude,
    };

    const assassins = try temp.alloc(admission.WireAssassin, level.assassins_len);
    for (level.assassins[0..level.assassins_len], 0..) |assassin, index| assassins[index] = .{
        .spawn_x = assassin.spawn_x.position,
        .ambush_count = assassin.ambush_count,
        .retreat_threshold = assassin.retreat_threshold,
    };

    const items = try temp.alloc(admission.WireWorldItem, level.items_len);
    for (level.items[0..level.items_len], 0..) |world_item, index| items[index] = .{
        .item = .{
            .id = try requireString(world_item.item.id),
            .kind = try itemKindToWire(world_item.item.kind),
            .name = try requireString(world_item.item.name),
            .weight = world_item.item.weight,
            .condition = conditionName(world_item.item.condition),
            .uses_remaining = if (world_item.item.has_uses_remaining) world_item.item.uses_remaining else null,
        },
        .world_x = world_item.world_x.position,
        .container = try requireString(world_item.container),
    };

    const wiring = try temp.alloc(admission.WireWiring, level.wiring_len);
    for (level.wiring[0..level.wiring_len], 0..) |challenge, index| wiring[index] = .{
        .kind = wiringName(challenge.kind),
        .device_ip = try ipText(temp, challenge.device_ip),
        .difficulty = challenge.difficulty,
    };

    const objective_count: usize = level.mission.objectives_len;
    const objectives = try temp.alloc(admission.WireObjective, objective_count);
    if (level.mission.objectives) |objective_ptr| {
        for (objective_ptr[0..objective_count], 0..) |objective, index| objectives[index] = .{
            .id = try requireString(objective.id),
            .description = try requireString(objective.description),
            .required = objective.required,
        };
    }

    const transitions = try temp.alloc(admission.WireTransition, level.zone_transitions_len);
    for (level.zone_transitions[0..level.zone_transitions_len], 0..) |transition, index| transitions[index] = .{
        .world_x = transition.world_x.position,
        .from_zone = try requireString(transition.from_zone),
        .to_zone = try requireString(transition.to_zone),
    };

    const defences = try temp.alloc(admission.WireDefence, level.device_defences_len);
    for (level.device_defences[0..level.device_defences_len], 0..) |defence, index| defences[index] = .{
        .ip = try ipText(temp, defence.ip),
        .flags = .{
            .tamper_proof = defence.flags.tamper_proof,
            .decoy = defence.flags.decoy,
            .canary = defence.flags.canary,
            .one_way_mirror = defence.flags.one_way_mirror,
            .kill_switch = defence.flags.kill_switch,
            .failover_target = if (defence.flags.failover_target.has_value) try ipText(temp, defence.flags.failover_target.ip) else null,
            .cascade_trap = if (defence.flags.cascade_trap.has_value) try ipText(temp, defence.flags.cascade_trap.ip) else null,
            .mirror_target = if (defence.flags.mirror_target.has_value) try ipText(temp, defence.flags.mirror_target.ip) else null,
            .instruction_whitelist = try whitelistToWire(temp, defence.flags.instruction_whitelist),
            .time_bomb = if (defence.flags.has_time_bomb) defence.flags.time_bomb else null,
            .undo_immunity = if (defence.flags.has_undo_immunity) defence.flags.undo_immunity else null,
        },
    };

    const wire = admission.WireLevel{
        .devices = devices,
        .zones = zones,
        .guards = guards,
        .dogs = dogs,
        .drones = drones,
        .assassins = assassins,
        .items = items,
        .wiring = wiring,
        .mission = .{
            .mission_id = try requireString(level.mission.mission_id),
            .location_id = try requireString(level.mission.location_id),
            .objectives = objectives,
            .time_limit = if (level.mission.has_time_limit) level.mission.time_limit else null,
        },
        .physical = .{
            .ground_y = level.physical.ground_y,
            .world_width = level.physical.world_width,
            .interaction_distance = level.physical.interaction_distance,
            .has_power_system = level.physical.has_power_system,
            .has_security_cameras = level.physical.has_security_cameras,
            .covert_links = level.physical.number_of_covert_links,
        },
        .zone_transitions = transitions,
        .device_defences = defences,
        .has_pbx = level.has_pbx,
        .pbx_ip = try ipText(temp, level.pbx_ip),
        .pbx_world_x = level.pbx_world_x.position,
    };
    try std.json.stringify(wire, .{ .emit_null_optional_fields = false }, writer);
}

fn parseIp(value: []const u8) CodecError!types.IpAddress {
    var parts = std.mem.splitScalar(u8, value, '.');
    var octets: [4]u8 = undefined;
    var index: usize = 0;
    while (parts.next()) |part| {
        if (part.len == 0 or index == octets.len) return error.InvalidIp;
        octets[index] = std.fmt.parseInt(u8, part, 10) catch return error.InvalidIp;
        index += 1;
    }
    if (index != octets.len) return error.InvalidIp;
    return types.IpAddress.init(octets[0], octets[1], octets[2], octets[3]);
}

fn namedEnum(comptime T: type, value: []const u8, names: []const []const u8) CodecError!T {
    for (names, 0..) |name, index| if (std.mem.eql(u8, name, value)) return @enumFromInt(index);
    return error.InvalidEnum;
}

fn itemKindFromWire(allocator: std.mem.Allocator, wire: admission.WireItemKind) !types.ItemKind {
    if (std.mem.eql(u8, wire.type, "cable")) return .{ .tag = .cable, .sub_type = @intFromEnum(try namedEnum(types.CableType, wire.sub_type.?, &.{ "ethernet", "fibre_lc", "fibre_sc", "serial", "usb", "universal" })), .capacity = 0, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "adapter")) return .{ .tag = .adapter, .sub_type = @intFromEnum(try namedEnum(types.AdapterType, wire.sub_type.?, &.{ "ethernet_to_fibre", "usb_to_serial", "media_converter" })), .capacity = 0, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "tool")) return .{ .tag = .tool, .sub_type = @intFromEnum(try namedEnum(types.ToolType, wire.sub_type.?, &.{ "crimper", "splicer", "multimeter", "wire_cutter", "debugger" })), .capacity = 0, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "module")) return .{ .tag = .module_, .sub_type = @intFromEnum(try namedEnum(types.ModuleType, wire.sub_type.?, &.{ "sfp", "gbic", "qsfp", "transceiver" })), .capacity = 0, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "storage")) return .{ .tag = .storage, .sub_type = 0, .capacity = wire.capacity.?, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "consumable")) return .{ .tag = .consumable, .sub_type = @intFromEnum(try namedEnum(types.ConsumableType, wire.sub_type.?, &.{ "battery_pack", "emp", "smoke_grenade", "decryptor" })), .capacity = 0, .zone_name = null };
    if (std.mem.eql(u8, wire.type, "keycard")) return .{ .tag = .keycard, .sub_type = 0, .capacity = 0, .zone_name = try dupeZ(allocator, wire.zone.?) };
    if (std.mem.eql(u8, wire.type, "radio")) return .{ .tag = .radio, .sub_type = 0, .capacity = 0, .zone_name = null };
    return error.InvalidEnum;
}

fn optionalIp(value: ?[]const u8) CodecError!types.OptionalIpAddress {
    return if (value) |ip| .some(try parseIp(ip)) else .none();
}

fn whitelistFromWire(allocator: std.mem.Allocator, value: ?[]const []const u8) !?[*:null]const ?[*:0]const u8 {
    const entries = value orelse return null;
    const result = try allocator.allocSentinel(?[*:0]const u8, entries.len, null);
    for (entries, 0..) |entry, index| result[index] = try dupeZ(allocator, entry);
    return result.ptr;
}

/// Deserialize a complete representable wire document into arena-owned C data.
pub fn deserializeLevelJson(backing: std.mem.Allocator, input: []const u8) !*types.LevelData {
    const owned = try createOwned(backing);
    errdefer {
        owned.arena.deinit();
        backing.destroy(owned);
    }
    const allocator = owned.arena.allocator();
    var parsed = try admission.parseRepresentableLevel(allocator, input);
    defer parsed.deinit();
    const wire = parsed.value;
    const mission = wire.mission orelse return error.MissingMission;
    const physical = wire.physical orelse return error.MissingPhysical;
    const level = &owned.level;

    for (wire.devices, 0..) |device, index| level.devices[index] = .{
        .kind = try namedEnum(types.DeviceKind, device.kind, &.{ "laptop", "desktop", "server", "router", "switch", "firewall", "camera", "access_point", "patch_panel", "power_supply", "phone_system", "fibre_hub" }),
        .ip = try parseIp(device.ip),
        .name = try dupeZ(allocator, device.name),
        .security = try namedEnum(types.SecurityLevel, device.security, &.{ "open", "weak", "medium", "strong" }),
    };
    level.devices_len = @intCast(wire.devices.len);

    for (wire.zones, 0..) |zone, index| level.zones[index] = .{ .name = try dupeZ(allocator, zone.name), .security_tier = zone.security_tier };
    level.zones_len = @intCast(wire.zones.len);
    for (wire.guards, 0..) |guard, index| level.guards[index] = .{
        .world_x = .{ .position = guard.world_x },
        .zone = try dupeZ(allocator, guard.zone),
        .rank = try namedEnum(types.GuardRank, guard.rank, &.{ "basic", "enforcer", "anti_hacker", "sentinel", "assassin", "elite", "security_chief", "rival_hacker" }),
        .patrol_radius = guard.patrol_radius,
    };
    level.guards_len = @intCast(wire.guards.len);
    for (wire.dogs, 0..) |dog, index| level.dogs[index] = .{ .world_x = .{ .position = dog.world_x }, .breed = try namedEnum(types.DogBreed, dog.breed, &.{ "patrol", "bloodhound", "robo_dog" }), .patrol_radius = dog.patrol_radius };
    level.dogs_len = @intCast(wire.dogs.len);
    for (wire.drones, 0..) |drone, index| level.drones[index] = .{ .world_x = .{ .position = drone.world_x }, .archetype = try namedEnum(types.DroneArchetype, drone.archetype, &.{ "helper", "hunter", "killer" }), .altitude = drone.altitude };
    level.drones_len = @intCast(wire.drones.len);
    for (wire.assassins, 0..) |assassin, index| level.assassins[index] = .{ .spawn_x = .{ .position = assassin.spawn_x }, .ambush_count = assassin.ambush_count, .retreat_threshold = assassin.retreat_threshold };
    level.assassins_len = @intCast(wire.assassins.len);

    for (wire.items, 0..) |world_item, index| level.items[index] = .{
        .item = .{
            .id = try dupeZ(allocator, world_item.item.id),
            .kind = try itemKindFromWire(allocator, world_item.item.kind),
            .name = try dupeZ(allocator, world_item.item.name),
            .weight = world_item.item.weight,
            .condition = try namedEnum(types.ItemCondition, world_item.item.condition, &.{ "pristine", "good", "worn", "damaged", "broken" }),
            .has_uses_remaining = world_item.item.uses_remaining != null,
            .uses_remaining = world_item.item.uses_remaining orelse 0,
        },
        .world_x = .{ .position = world_item.world_x },
        .container = try dupeZ(allocator, world_item.container),
    };
    level.items_len = @intCast(wire.items.len);
    for (wire.wiring, 0..) |challenge, index| level.wiring[index] = .{
        .kind = try namedEnum(types.WiringType, challenge.kind, &.{ "patch_panel", "switch_backplane", "server_rack", "fibre_splicing", "pbx_comms" }),
        .device_ip = try parseIp(challenge.device_ip),
        .difficulty = challenge.difficulty,
    };
    level.wiring_len = @intCast(wire.wiring.len);

    const objectives = try allocator.alloc(types.MissionObjective, mission.objectives.len);
    for (mission.objectives, 0..) |objective, index| objectives[index] = .{
        .id = try dupeZ(allocator, objective.id),
        .description = try dupeZ(allocator, objective.description),
        .required = objective.required,
    };
    level.mission = .{
        .mission_id = try dupeZ(allocator, mission.mission_id),
        .location_id = try dupeZ(allocator, mission.location_id),
        .objectives = if (objectives.len == 0) null else objectives.ptr,
        .objectives_len = @intCast(objectives.len),
        .has_time_limit = mission.time_limit != null,
        .time_limit = mission.time_limit orelse 0,
    };
    level.physical = .{
        .ground_y = physical.ground_y,
        .world_width = physical.world_width,
        .interaction_distance = physical.interaction_distance,
        .has_power_system = physical.has_power_system,
        .has_security_cameras = physical.has_security_cameras,
        .number_of_covert_links = physical.covert_links,
    };

    for (wire.zone_transitions, 0..) |transition, index| level.zone_transitions[index] = .{
        .world_x = .{ .position = transition.world_x },
        .from_zone = try dupeZ(allocator, transition.from_zone),
        .to_zone = try dupeZ(allocator, transition.to_zone),
    };
    level.zone_transitions_len = @intCast(wire.zone_transitions.len);
    for (wire.device_defences, 0..) |defence, index| level.device_defences[index] = .{
        .ip = try parseIp(defence.ip),
        .flags = .{
            .tamper_proof = defence.flags.tamper_proof,
            .decoy = defence.flags.decoy,
            .canary = defence.flags.canary,
            .one_way_mirror = defence.flags.one_way_mirror,
            .kill_switch = defence.flags.kill_switch,
            .failover_target = try optionalIp(defence.flags.failover_target),
            .cascade_trap = try optionalIp(defence.flags.cascade_trap),
            .mirror_target = try optionalIp(defence.flags.mirror_target),
            .instruction_whitelist = try whitelistFromWire(allocator, defence.flags.instruction_whitelist),
            .has_time_bomb = defence.flags.time_bomb != null,
            .time_bomb = defence.flags.time_bomb orelse 0,
            .has_undo_immunity = defence.flags.undo_immunity != null,
            .undo_immunity = defence.flags.undo_immunity orelse 0,
        },
    };
    level.device_defences_len = @intCast(wire.device_defences.len);
    level.has_pbx = wire.has_pbx;
    level.pbx_ip = try parseIp(wire.pbx_ip);
    level.pbx_world_x = .{ .position = wire.pbx_world_x };
    return level;
}
