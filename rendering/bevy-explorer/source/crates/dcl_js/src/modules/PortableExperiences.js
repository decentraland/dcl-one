module.exports.spawn = async function (body) { 
    // console.log("PortableExperiences.spawn", body)

    let res = await Deno.core.ops.op_portable_spawn(body.pid, body.ens);

    // console.log("spawn -> ", res);

    return res;
}
module.exports.kill = async function (body) { 
    // console.log("PortableExperiences.kill", body)
    let res = await Deno.core.ops.op_portable_kill(body.pid);
    return {
        status: res
    }
}
module.exports.exit = async function (body) { 
    let res = await Deno.core.ops.op_portable_exit();
    return {
        status: res
    }
}
module.exports.getPortableExperiencesLoaded = async function (body) { 
    // console.log("PortableExperiences.getPortableExperiencesLoaded", body);
    let res = await Deno.core.ops.op_portable_list();
    // console.log("PortableExperiences.getPortableExperiencesLoaded => ", res);
    return {
        loaded: res
    }
}