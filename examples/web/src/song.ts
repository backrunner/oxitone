import { Pattern, Project, wavetable } from "@oxitone/web";
export function createSong(sampleRate:number):Project {
  const p=new Project({name:"Glass after rain",sampleRate,seed:20260907});
  p.setTempo(100);
  const send=p.addMixerChannel({name:"Space",inserts:[{pluginId:"oxitone.reverb",pluginVersion:"1.0.0",parameters:{decaySeconds:1.8,damping:0.6}}]});
  const keys=p.addMixerChannel({name:"Glass",inserts:[{pluginId:"oxitone.delay",pluginVersion:"1.0.0",parameters:{timeBeats:0.75,feedback:0.3},mix:0.2}]});
  keys.send(send,{ratio:0.16});
  const lead=p.addChannel({name:"Glass keys",mixerChannelId:keys.id,level:0.27,instrument:wavetable({
    oscA:{wave:"triangle",morphTo:"glass",position:0.3},oscB:{wave:"sine"},mix:0.2,
    amp:{attack:0.008,decay:0.55,sustain:0.25,release:0.4},filter:{cutoff:4200},
  })});
  const bass=p.addChannel({name:"Sub",level:0.28,instrument:wavetable({oscA:{wave:"sine"},amp:{release:0.12}})});
  const drums=p.addChannel({name:"Drum machine",level:0.65,instrument:{pluginId:"example.drums",pluginVersion:"1.0.0",parameters:{volume:0.75,decay:0.85}}});
  const lt=p.addTrack("Glass motif").use(lead),bt=p.addTrack("Bass").use(bass),dt=p.addTrack("Drums").use(drums);
  const melody=[74,77,81,79,77,74,72,69], roots=[38,34,41,36];
  for(let bar=0;bar<16;bar++){
    lt.add(new Pattern({id:`pat_lead_${bar}`,lengthBeats:4,notes:melody.map((pitch,i)=>({pitch:pitch+(bar%4===3?-2:0),start:i*0.5,duration:i===7?0.45:0.3,velocity:i%2?0.65:0.85}))})).at({bar:bar+1});
    bt.add(new Pattern({id:`pat_bass_${bar}`,lengthBeats:4,notes:[0,2].map(start=>({pitch:roots[bar%4]!,start,duration:1.4,velocity:0.8}))})).at({bar:bar+1});
    dt.add(new Pattern({id:`pat_drums_${bar}`,lengthBeats:4,notes:[
      ...[0,2,2.75].map(start=>({pitch:36,start,duration:0.1,velocity:0.85})),
      ...[1,3].map(start=>({pitch:38,start,duration:0.1,velocity:0.8})),
      ...Array.from({length:8},(_,i)=>({pitch:42,start:i*0.5,duration:0.08,velocity:i%2?0.35:0.5})),
    ]})).at({bar:bar+1});
  }
  p.master.level=0.8;
  return p;
}
