use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct CityCoord {
    pub name: &'static str,
    pub keywords: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub tz_offset: f64,
    pub tz_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrayerRegion {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub tz_offset: f64,
    pub tz_label: String,
}

impl CityCoord {
    pub fn to_region(&self) -> PrayerRegion {
        PrayerRegion {
            name: self.name.to_string(),
            lat: self.lat,
            lon: self.lon,
            tz_offset: self.tz_offset,
            tz_label: self.tz_label.to_string(),
        }
    }
}

pub const INDONESIAN_CITIES: &[CityCoord] = &[
    // === DKI JAKARTA & KECAMATAN ===
    CityCoord {
        name: "Jakarta",
        keywords: "dki jakarta pusat gambir tanah abang menteng senen cempaka putih johar baru kemayoran sawah besar",
        lat: -6.2088,
        lon: 106.8456,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jakarta Selatan",
        keywords: "jaksel kebayoran baru kebayoran lama pesanggrahan cilandak pasar minggu jagakarsa mampang prapatan pancoran tebet setiabudi",
        lat: -6.2615,
        lon: 106.8106,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jakarta Timur",
        keywords: "jaktim matraman pulogadung jatinegara duren sawit kramat jati makasar pasar rebo ciracas cipayung cakung",
        lat: -6.2250,
        lon: 106.9004,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jakarta Barat",
        keywords: "jakbar cengkareng grogol petamburan taman sari tambora kebon jeruk kalideres palmerah kembangan",
        lat: -6.1674,
        lon: 106.7637,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jakarta Utara",
        keywords: "jakut penjaringan pademangan tanjung priok koja kelapa gading cilincing",
        lat: -6.1384,
        lon: 106.8639,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === BODETABEK & BANTEN ===
    CityCoord {
        name: "Bogor",
        keywords: "kota bogor kab bogor cibinong bogor selatan timur utara tengah barat tanah sareal ciawi sentul parung gunung putri cileungsi",
        lat: -6.5971,
        lon: 106.8060,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Depok",
        keywords: "kota depok beji pancoran mas cipayung sukmajaya cilodong limo cinere cimanggis tapos sawangan bojongsari margonda",
        lat: -6.4025,
        lon: 106.7942,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Tangerang",
        keywords: "kota tangerang ciledug karang tengah cipondoh pinang taman royal batuceper benda jatiuwung cibodas negalsari periuk karawaci",
        lat: -6.1783,
        lon: 106.6319,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Tangerang Selatan",
        keywords: "tangsel serpong serpong utara ciputat ciputat timur pondok aren pamulang setu bsd bintaro alam sutera",
        lat: -6.2886,
        lon: 106.7179,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Bekasi",
        keywords: "kota bekasi kab bekasi cikarang pondok gede jatiasih jatisampurna bekasi barat timur utara selatan medan satria mustika jaya rawalumbu tambun",
        lat: -6.2383,
        lon: 106.9756,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Serang",
        keywords: "kota serang banten cipocok jaya curug kasemen тактакан walantaka cilegon pandeglang lebak rangkasbitung",
        lat: -6.1104,
        lon: 106.1503,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Cilegon",
        keywords: "kota cilegon merak pulomerak cibeber citangkil ciwandan gerogol jombang purwakarta",
        lat: -6.0025,
        lon: 106.0111,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === JAWA BARAT & KECAMATAN ===
    CityCoord {
        name: "Bandung",
        keywords: "kota bandung coblong dago sukasari cidadap andir cicendo lengkong regol batununggal kiaracondong buahbatu rancasari gedebage cibiru ujungberung antapani arcamanik",
        lat: -6.9175,
        lon: 107.6191,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Kab. Bandung (Soreang/Bojongsoang)",
        keywords: "kabupaten bandung soreang bojongsoang dayeuhkolot baleendah banjaran cileunyi rancaekek majalaya margahayu katapang ciparay pangalengan ciwidey",
        lat: -7.0253,
        lon: 107.5198,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Cimahi & Bandung Barat",
        keywords: "kota cimahi cimahi utara tengah selatan lembang padalarang parongpong ngamprah batujajar cisarua",
        lat: -6.8722,
        lon: 107.5425,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Sumedang",
        keywords: "sumedang jatinangor tanjungsari sumedang utara selatan cimalaka",
        lat: -6.8381,
        lon: 107.9275,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Garut",
        keywords: "garut tarogong kidul tarogong kaler garut kota karangpawitan leles kadungora cikajang",
        lat: -7.2279,
        lon: 107.9087,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Tasikmalaya",
        keywords: "kota tasikmalaya cihideung cipedes tawang indihiang kawalu ciawi singaparna ciamis banjar pangandaran",
        lat: -7.3274,
        lon: 108.2207,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Cirebon",
        keywords: "kota cirebon kejaksan kesambi pekalipan harjamukti lemahwungkuk sumber palimanan indramayu majalengka kuningan",
        lat: -6.7320,
        lon: 108.5523,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Karawang & Purwakarta",
        keywords: "karawang barat timur telukjambe cikampek klari purwakarta subang",
        lat: -6.3063,
        lon: 107.3019,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Sukabumi & Cianjur",
        keywords: "kota sukabumi cikole citamiang warudoyong gunungpuyuh cisaat cibadak palabuhanratu cianjur cipanas",
        lat: -6.9277,
        lon: 106.9300,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === JAWA TENGAH & DIY ===
    CityCoord {
        name: "Semarang",
        keywords: "kota semarang banyumanik tembalang pedurungan gayamsari genuk semarang tengah selatan barat timur utara gunungpati mijen ngaliyan tugu ungaran",
        lat: -6.9667,
        lon: 110.4167,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Yogyakarta",
        keywords: "jogja kota yogyakarta gondokusuman umbulharjo kotagede بايرو danurejan gedongtengen jetis kraton mantrijeron mergangsan ngampilan pakualaman tegalrejo wirobrajan",
        lat: -7.7956,
        lon: 110.3695,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Sleman & Bantul (DIY)",
        keywords: "sleman depok mlati gamping ngaglik gbulaksumur caturtunggal condongcatur kalasan prambanan bantul kasihan sewon banguntapan kulon progo wates gunungkidul wonosari",
        lat: -7.7161,
        lon: 110.3550,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Surakarta (Solo)",
        keywords: "solo surakarta laweyan serengan pasar kliwon jebres banjarsari kartasura sukoharjo karanganyar boyolali sragen klaten wonogiri",
        lat: -7.5755,
        lon: 110.8243,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Magelang & Salatiga",
        keywords: "kota magelang mertoyudan mungkid borobudur salatiga sidorejo tingkir argomulyo sidomukti temanggung wonosobo",
        lat: -7.4797,
        lon: 110.2177,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Purwokerto (Banyumas)",
        keywords: "purwokerto banyumas sokaraja baturraden cilacap purbalingga banjarnegara kebumen",
        lat: -7.4244,
        lon: 109.2302,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Pekalongan & Tegal",
        keywords: "kota pekalongan kajen batang pemalang kota tegal slawi brebes bumiayu",
        lat: -6.8886,
        lon: 109.6753,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Kudus, Jepara & Pati",
        keywords: "kudus jepara pati demak rembang blora cepu grobogan purwodadi kendal",
        lat: -6.8048,
        lon: 110.8405,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === JAWA TIMUR ===
    CityCoord {
        name: "Surabaya",
        keywords: "kota surabaya gubeng wonokromo rungkut sukolilo mulyorejo tambaksari genteng tegalsari sawahan sukomanunggal tandes lakarsantri wiyung gayungan jambangan wonocolo kenjeran",
        lat: -7.2575,
        lon: 112.7521,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Sidoarjo & Gresik",
        keywords: "sidoarjo waru taman gedangan candi buduran krian gresik kebomas manyar menganti driyorejo",
        lat: -7.4478,
        lon: 112.7183,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Malang & Batu",
        keywords: "kota malang lowokwaru klojen blimbing kedungkandang sukun singosari lawang kepanjen kota batu bumiaji junrejo",
        lat: -7.9666,
        lon: 112.6326,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Kediri, Blitar & Tulungagung",
        keywords: "kota kediri mojoroto pesantren pare kota blitar tulungagung trenggalek nganjuk jombang mojokerto",
        lat: -7.8480,
        lon: 112.0178,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Madiun, Ponorogo & Ngawi",
        keywords: "kota madiun kartoharjo mangunharjo taman ponorogo magetan ngawi pacitan bojonegoro tuban lamongan",
        lat: -7.6298,
        lon: 111.5239,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jember & Banyuwangi",
        keywords: "jember sumbersari patrang kaliwates ambulu banyuwangi genteng rogojampi muncar bondowoso situbondo lumajang probolinggo pasuruan",
        lat: -8.1845,
        lon: 113.6681,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Madura (Bangkalan/Pamekasan/Sumenep)",
        keywords: "madura bangkalan sampang pamekasan sumenep kamal burneh",
        lat: -7.1568,
        lon: 113.4746,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === SUMATERA (WIB) ===
    CityCoord {
        name: "Banda Aceh & Lhokseumawe",
        keywords: "banda aceh baiturrahman kuta alam syiah kuala meuraxa aceh besar lhokseumawe langsa meulaboh sabang bireuen",
        lat: 5.5483,
        lon: 95.3238,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Medan",
        keywords: "kota medan medan baru medan selayang medan sunggal medan johor medan tembung medan denai medan helvetia medan petisah medan area deli serdang binjai tebing tinggi pematangsiantar",
        lat: 3.5952,
        lon: 98.6722,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Padang & Bukittinggi",
        keywords: "kota padang koto tangah padang utara barat selatan timur nanggalo kuranji pauh lubuk begalung bukittinggi payakumbuh pariaman solok",
        lat: -0.9471,
        lon: 100.4172,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Pekanbaru",
        keywords: "kota pekanbaru riau tampan tuah madani binawidya marpoyan damai bukit raya tenayan raya sukajadi payung sekaki rumbai dumai bengkalis kampar",
        lat: 0.5071,
        lon: 101.4478,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Batam & Tanjungpinang",
        keywords: "kota batam kepulauan riau kepri batam kota lubuk baja sekupang batu aji bengkong nongsa sagulung sei beduk tanjungpinang bintan karimun",
        lat: 1.0456,
        lon: 104.0305,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Jambi",
        keywords: "kota jambi kota baru alam barajo telanaipura jambi selatan paal merah pasar jambi muaro jambi bungo merangin",
        lat: -1.6101,
        lon: 103.6131,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Palembang",
        keywords: "kota palembang sumatera selatan ilir timur ilir barat seberang ulu sukarami alang-alang lebar kemuning kalidoni sako plaju jakabaring prabumulih lubuklinggau baturaja",
        lat: -2.9761,
        lon: 104.7754,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Bengkulu",
        keywords: "kota bengkulu ratu agung ratu samban teluk segara gading cempaka selebar muara bangkahulu curup rejang lebong",
        lat: -3.7928,
        lon: 102.2608,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Bandar Lampung & Metro",
        keywords: "bandar lampung rajabasa kedaton sukarame tanjung karang way halim kemiling teluk betung panjang metro lampung selatan kalianda pringsewu",
        lat: -5.3971,
        lon: 105.2668,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Pangkalpinang (Bangka Belitung)",
        keywords: "pangkalpinang bangka belitung sungai liat mentok tanjung pandan manggar",
        lat: -2.1316,
        lon: 106.1169,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === KALIMANTAN BARAT & TENGAH (WIB) ===
    CityCoord {
        name: "Pontianak & Singkawang",
        keywords: "kota pontianak kalimantan barat pontianak kota selatan timur barat utara tenggara kubu raya sungai raya singkawang ketapang sambas sintang",
        lat: -0.0263,
        lon: 109.3425,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    CityCoord {
        name: "Palangka Raya",
        keywords: "kota palangka raya kalimantan tengah jekan raya pahandut sabangau bukit batu sampit kotawaringin pangkalan bun kapuas",
        lat: -2.2161,
        lon: 113.9140,
        tz_offset: 7.0,
        tz_label: "WIB",
    },
    // === KALIMANTAN SELATAN, TIMUR & UTARA (WITA) ===
    CityCoord {
        name: "Banjarmasin & Banjarbaru",
        keywords: "kota banjarmasin kalimantan selatan banjarmasin tengah utara selatan timur barat banjarbaru martapura tanah laut kotabaru",
        lat: -3.3194,
        lon: 114.5908,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Balikpapan",
        keywords: "kota balikpapan kalimantan timur balikpapan selatan tengah utara barat timur kota penajam paser utara ikn nusantara",
        lat: -1.2379,
        lon: 116.8529,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Samarinda & Bontang",
        keywords: "kota samarinda samarinda ulu ilir utara seberang sungai kunjang sambutan palaran loa janan tenggarong kutai kartanegara bontang sangatta",
        lat: -0.5022,
        lon: 117.1536,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Tarakan & Tanjung Selor",
        keywords: "kota tarakan kalimantan utara tanjung selor bulungan nunukan malinau",
        lat: 3.3000,
        lon: 117.6333,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    // === BALI, NTB & NTT (WITA) ===
    CityCoord {
        name: "Denpasar & Badung (Bali)",
        keywords: "kota denpasar bali denpasar selatan timur barat utara badung kuta kuta selatan utara mengwi abiansemal jimbaran nusa dua canggu seminyak ubud gianyar tabanan buleleng singaraja",
        lat: -8.6705,
        lon: 115.2126,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Mataram & Lombok",
        keywords: "kota mataram nusa tenggara barat ntb ampenan cakranegara mataram selaparang sandubaya sekarbela lombok barat tengah timur praya selong bima sumbawa",
        lat: -8.5833,
        lon: 116.1167,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Kupang",
        keywords: "kota kupang nusa tenggara timur ntt oebobo kelapa lima maulafa alak kota raja kota lama labuan bajo flores ende maumere waingapu",
        lat: -10.1772,
        lon: 123.6070,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    // === SULAWESI (WITA) ===
    CityCoord {
        name: "Makassar",
        keywords: "kota makassar sulawesi selatan panakkukang rappocini tamalanrea biringkanaya manggala mariso mamajang makassar ujung pandang tanah tallo bontoala wajo gowa sungguminasa maros",
        lat: -5.1477,
        lon: 119.4327,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Parepare & Palopo",
        keywords: "kota parepare bacukiki ujung soreang kota palopo wara bone watampone bulukumba bantaeng jeneponto takalar pangkep barru sidrap pinrang enrekang toraja",
        lat: -4.0135,
        lon: 119.6255,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Kendari",
        keywords: "kota kendari sulawesi tenggara mandonga kambu poasia baruga wua-wua kadia puuwatu kendari barat abeli nambo konawe unaaha kolaka baubau buton muna raha",
        lat: -3.9985,
        lon: 122.5130,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Baubau & Kolaka (Sultra)",
        keywords: "kota baubau wolio betoambari murhum kokalukuna kolaka pomalaa wakatobi wangi-wangi bombana konawe selatan andoolo",
        lat: -5.4633,
        lon: 122.6019,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Palu",
        keywords: "kota palu sulawesi tengah palu selatan timur barat utara mantikulore tatanga ulujadi tawaeli sigi donggala poso luwuk banggai toli-toli morowali",
        lat: -0.8917,
        lon: 119.8707,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Manado",
        keywords: "kota manado sulawesi utara malalayang wanea sario wenang tikala paal dua mapanget tuminting singkil bunaken bitung tomohon minahasa tondano kotamobagu",
        lat: 1.4748,
        lon: 124.8421,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Gorontalo",
        keywords: "kota gorontalo kota selatan timur utara barat tengah dungingi dumbo raya hulonthalangi sipatana limboto bone bolango",
        lat: 0.5435,
        lon: 123.0568,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    CityCoord {
        name: "Mamuju (Sulawesi Barat)",
        keywords: "mamuju sulawesi barat simboro kalukku polewali mandar wonomulyo majene pasangkayu",
        lat: -2.6748,
        lon: 118.8885,
        tz_offset: 8.0,
        tz_label: "WITA",
    },
    // === MALUKU & PAPUA (WIT) ===
    CityCoord {
        name: "Ambon",
        keywords: "kota ambon maluku sirimau nusaniwe baguala teluk ambon leitimur selatan masohi tual maluku tenggara buru namlea",
        lat: -3.6954,
        lon: 128.1814,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
    CityCoord {
        name: "Ternate & Tidore",
        keywords: "kota ternate maluku utara ternate selatan tengah utara pulau ternate tidore kepulauan soasiu sofifi halmahera tobelo bacan",
        lat: 0.7893,
        lon: 127.3774,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
    CityCoord {
        name: "Sorong & Manokwari",
        keywords: "kota sorong papua barat daya sorong timur barat utara kota manokwari papua barat fakfak kaimana raja ampat waisai bintuni",
        lat: -0.8762,
        lon: 131.2558,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
    CityCoord {
        name: "Jayapura",
        keywords: "kota jayapura papua abepura heram jayapura selatan utara muara tami sentani biak nabire timika mimika wamena merauke",
        lat: -2.5916,
        lon: 140.6690,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
    CityCoord {
        name: "Timika & Merauke",
        keywords: "timika mimika baru kuala kencana tembagapura merauke papua selatan papua tengah nabire",
        lat: -4.5468,
        lon: 136.8837,
        tz_offset: 9.0,
        tz_label: "WIT",
    },
];

#[derive(Clone, Debug)]
pub struct PrayerSchedule {
    pub city: String,
    pub tz_label: String,
    pub prayers: Vec<(String, String, u32)>, // (Name, HH:MM, minutes_from_midnight)
    pub next_prayer_name: String,
    pub next_prayer_time: String,
    pub countdown_str: String,
}

fn deg2rad(d: f64) -> f64 {
    d * std::f64::consts::PI / 180.0
}

fn rad2deg(r: f64) -> f64 {
    r * 180.0 / std::f64::consts::PI
}

fn hour_angle(lat: f64, decl: f64, altitude_deg: f64) -> f64 {
    let lat_r = deg2rad(lat);
    let decl_r = deg2rad(decl);
    let alt_r = deg2rad(altitude_deg);
    let cos_ha = (alt_r.sin() - lat_r.sin() * decl_r.sin()) / (lat_r.cos() * decl_r.cos());
    let clamped = cos_ha.clamp(-1.0, 1.0);
    rad2deg(clamped.acos()) / 15.0
}

fn format_hours_to_hm(hours: f64) -> (String, u32) {
    let mut total_mins = (hours * 60.0).round() as i32;
    total_mins = total_mins.rem_euclid(24 * 60);
    let h = (total_mins / 60) as u32;
    let m = (total_mins % 60) as u32;
    (format!("{:02}:{:02}", h, m), (h * 60 + m))
}

pub fn infer_timezone(lat: f64, lon: f64, display_name: &str) -> (f64, &'static str) {
    let lower = display_name.to_lowercase();
    if lower.contains("papua")
        || lower.contains("maluku")
        || lower.contains("ambon")
        || lower.contains("jayapura")
        || lower.contains("sorong")
        || lower.contains("ternate")
        || lower.contains("merauke")
        || lower.contains("manokwari")
        || lower.contains("timika")
    {
        return (9.0, "WIT");
    }
    if lower.contains("bali")
        || lower.contains("nusa tenggara")
        || lower.contains("sulawesi")
        || lower.contains("gorontalo")
        || lower.contains("kalimantan selatan")
        || lower.contains("kalimantan timur")
        || lower.contains("kalimantan utara")
        || lower.contains("makassar")
        || lower.contains("kendari")
        || lower.contains("denpasar")
        || lower.contains("balikpapan")
        || lower.contains("samarinda")
        || lower.contains("banjarmasin")
        || lower.contains("manado")
        || lower.contains("palu")
        || lower.contains("mataram")
        || lower.contains("kupang")
    {
        return (8.0, "WITA");
    }
    if lower.contains("kalimantan barat")
        || lower.contains("kalimantan tengah")
        || lower.contains("pontianak")
        || lower.contains("palangka")
        || lower.contains("jawa")
        || lower.contains("jakarta")
        || lower.contains("banten")
        || lower.contains("yogyakarta")
        || lower.contains("sumatera")
        || lower.contains("sumatra")
        || lower.contains("aceh")
        || lower.contains("riau")
        || lower.contains("jambi")
        || lower.contains("bengkulu")
        || lower.contains("lampung")
        || lower.contains("bangka")
    {
        return (7.0, "WIB");
    }

    let _ = lat;
    if lon >= 127.0 {
        (9.0, "WIT")
    } else if lon >= 114.3 {
        (8.0, "WITA")
    } else {
        (7.0, "WIB")
    }
}

pub fn resolve_region_from_state(
    selected_city: &str,
    lat_opt: Option<f64>,
    lon_opt: Option<f64>,
    tz_offset_opt: Option<f64>,
    tz_label_opt: Option<&str>,
) -> PrayerRegion {
    if let (Some(lat), Some(lon)) = (lat_opt, lon_opt) {
        let (def_off, def_lbl) = infer_timezone(lat, lon, selected_city);
        let tz_offset = tz_offset_opt.unwrap_or(def_off);
        let tz_label = tz_label_opt
            .filter(|s| !s.is_empty())
            .unwrap_or(def_lbl)
            .to_string();
        return PrayerRegion {
            name: if selected_city.trim().is_empty() {
                "Jakarta".to_string()
            } else {
                selected_city.to_string()
            },
            lat,
            lon,
            tz_offset,
            tz_label,
        };
    }

    let q = selected_city.trim();
    if let Some(exact) = INDONESIAN_CITIES
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(q))
    {
        return exact.to_region();
    }

    let local_matches = search_regions_local(q);
    if let Some(first) = local_matches.into_iter().next() {
        return first;
    }

    INDONESIAN_CITIES[0].to_region()
}

pub fn search_regions_local(query: &str) -> Vec<PrayerRegion> {
    let clean = query
        .trim()
        .to_lowercase()
        .replace("kecamatan", "")
        .replace("kec.", "")
        .replace("kabupaten", "")
        .replace("kab.", "")
        .replace("kota", "")
        .replace(',', " ");
    let tokens: Vec<&str> = clean.split_whitespace().filter(|t| !t.is_empty()).collect();

    if tokens.is_empty() {
        return INDONESIAN_CITIES.iter().map(|c| c.to_region()).collect();
    }

    let mut exact_name_matches = Vec::new();
    let mut keyword_matches = Vec::new();

    for c in INDONESIAN_CITIES {
        let name_lower = c.name.to_lowercase();
        let hay = format!("{} {}", name_lower, c.keywords.to_lowercase());
        if tokens.iter().all(|t| hay.contains(t)) {
            if tokens.iter().all(|t| name_lower.contains(t)) {
                exact_name_matches.push(c.to_region());
            } else {
                // Format with the matched subdistrict/kecamatan name so the user sees their specific Kecamatan!
                let mut reg = c.to_region();
                let raw_trimmed = query.trim();
                if !raw_trimmed.is_empty() && !name_lower.contains(&raw_trimmed.to_lowercase()) {
                    let cap = capitalize_words(raw_trimmed);
                    reg.name = format!("{}, {}", cap, c.name);
                }
                keyword_matches.push(reg);
            }
        }
    }

    exact_name_matches.extend(keyword_matches);
    exact_name_matches
}

fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Deserialize)]
struct NominatimPlace {
    lat: String,
    lon: String,
    display_name: Option<String>,
    name: Option<String>,
    address: Option<NominatimAddress>,
}

#[derive(Deserialize)]
struct NominatimAddress {
    suburb: Option<String>,
    village: Option<String>,
    neighbourhood: Option<String>,
    municipality: Option<String>,
    city_district: Option<String>,
    town: Option<String>,
    city: Option<String>,
    county: Option<String>,
    state: Option<String>,
}

pub fn search_regions_online(query: &str) -> Vec<PrayerRegion> {
    let q = query.trim();
    if q.len() < 2 {
        return Vec::new();
    }

    let out = Command::new("curl")
        .args([
            "-4",
            "--connect-timeout",
            "3",
            "--max-time",
            "5",
            "-sG",
            "-H",
            "User-Agent: Arwiland/0.1 (Linux Dynamic Island; Prayer Schedule Lookup)",
            "-H",
            "Accept-Language: id,en",
            "https://nominatim.openstreetmap.org/search",
            "--data-urlencode",
            &format!("q={}", q),
            "-d",
            "format=json&addressdetails=1&limit=8&countrycodes=id",
        ])
        .output();

    let Ok(output) = out else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let Ok(places) = serde_json::from_slice::<Vec<NominatimPlace>>(&output.stdout) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for p in places {
        let Ok(lat) = p.lat.parse::<f64>() else {
            continue;
        };
        let Ok(lon) = p.lon.parse::<f64>() else {
            continue;
        };
        let full_disp = p.display_name.clone().unwrap_or_default();
        let label = format_nominatim_label(&p, q);
        if label.is_empty() {
            continue;
        }
        let (tz_offset, tz_label) = infer_timezone(lat, lon, &full_disp);
        if !results
            .iter()
            .any(|r: &PrayerRegion| r.name.eq_ignore_ascii_case(&label))
        {
            results.push(PrayerRegion {
                name: label,
                lat,
                lon,
                tz_offset,
                tz_label: tz_label.to_string(),
            });
        }
    }
    results
}

fn format_nominatim_label(p: &NominatimPlace, fallback_q: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref n) = p.name {
        if !n.trim().is_empty() {
            parts.push(n.trim().to_string());
        }
    }
    if let Some(ref addr) = p.address {
        let sub = addr
            .municipality
            .as_ref()
            .or(addr.city_district.as_ref())
            .or(addr.suburb.as_ref())
            .or(addr.village.as_ref())
            .or(addr.town.as_ref())
            .or(addr.neighbourhood.as_ref());
        if let Some(s) = sub {
            let s_clean = s.trim();
            if !s_clean.is_empty()
                && !parts
                    .iter()
                    .any(|x| x.to_lowercase().contains(&s_clean.to_lowercase()))
            {
                parts.push(s_clean.to_string());
            }
        }
        let city = addr.city.as_ref().or(addr.county.as_ref());
        if let Some(c) = city {
            let c_clean = c.trim();
            if !c_clean.is_empty()
                && !parts
                    .iter()
                    .any(|x| x.to_lowercase().contains(&c_clean.to_lowercase()))
            {
                parts.push(c_clean.to_string());
            }
        }
        if parts.len() < 2 {
            if let Some(ref st) = addr.state {
                let st_clean = st.trim();
                if !st_clean.is_empty()
                    && !parts
                        .iter()
                        .any(|x| x.to_lowercase().contains(&st_clean.to_lowercase()))
                {
                    parts.push(st_clean.to_string());
                }
            }
        }
    }

    if parts.is_empty() {
        if let Some(ref d) = p.display_name {
            let segs: Vec<&str> = d.split(',').map(|s| s.trim()).take(2).collect();
            if !segs.is_empty() {
                return segs.join(", ");
            }
        }
        return capitalize_words(fallback_q);
    }

    parts.into_iter().take(2).collect::<Vec<_>>().join(", ")
}

#[allow(dead_code)]
pub fn calculate_prayer_schedule(city_name: &str) -> PrayerSchedule {
    let region = resolve_region_from_state(city_name, None, None, None, None);
    calculate_prayer_schedule_for_region(&region)
}

pub fn calculate_prayer_schedule_for_region(region: &PrayerRegion) -> PrayerSchedule {
    let now = Local::now();
    let day_of_year = now.ordinal() as f64;

    // Solar approximation (accurate within ~1-2 minutes for Kemenag RI standard)
    let gamma = 2.0 * std::f64::consts::PI / 365.0 * (day_of_year - 1.0);
    let eq_time_min = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2.0 * gamma).cos()
            - 0.040849 * (2.0 * gamma).sin());
    let decl_rad = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin()
        - 0.002697 * (3.0 * gamma).cos()
        + 0.00148 * (3.0 * gamma).sin();
    let decl_deg = rad2deg(decl_rad);

    // Solar noon + 2 minute ihtiyath (Kemenag standard safety margin)
    let ihtiyath_hours = 2.0 / 60.0;
    let noon = 12.0 + region.tz_offset - (region.lon / 15.0) - (eq_time_min / 60.0);

    let dzuhur_h = noon + ihtiyath_hours;
    let fajr_ha = hour_angle(region.lat, decl_deg, -20.0);
    let subuh_h = noon - fajr_ha + ihtiyath_hours;
    let imsak_h = subuh_h - (10.0 / 60.0);

    // Ashar (Shafi'i shadow factor = 1 + tan(|lat - decl|))
    let shadow_angle_rad = (1.0 / (1.0 + deg2rad((region.lat - decl_deg).abs()).tan())).atan();
    let ashar_ha = hour_angle(region.lat, decl_deg, rad2deg(shadow_angle_rad));
    let ashar_h = noon + ashar_ha + ihtiyath_hours;

    // Maghrib (-0.833 deg sunset)
    let maghrib_ha = hour_angle(region.lat, decl_deg, -0.833);
    let maghrib_h = noon + maghrib_ha + ihtiyath_hours;

    // Isya (-18 deg)
    let isya_ha = hour_angle(region.lat, decl_deg, -18.0);
    let isya_h = noon + isya_ha + ihtiyath_hours;

    let raw_list = [
        ("Imsak", imsak_h),
        ("Subuh", subuh_h),
        ("Dzuhur", dzuhur_h),
        ("Ashar", ashar_h),
        ("Maghrib", maghrib_h),
        ("Isya", isya_h),
    ];

    let mut prayers = Vec::new();
    for (label, h) in raw_list {
        let (hm, mins) = format_hours_to_hm(h);
        prayers.push((label.to_string(), hm, mins));
    }

    let now_secs = now.hour() * 3600 + now.minute() * 60 + now.second();
    let mut next_idx = 0;
    let mut found_today = false;
    for (i, (_, _, mins)) in prayers.iter().enumerate() {
        if (*mins * 60) > now_secs {
            next_idx = i;
            found_today = true;
            break;
        }
    }

    let (next_name, next_time, next_mins) = &prayers[next_idx];
    let target_secs = if found_today {
        next_mins * 60
    } else {
        (next_mins + 24 * 60) * 60
    };
    let rem_secs = target_secs.saturating_sub(now_secs);
    let rh = rem_secs / 3600;
    let rm = (rem_secs % 3600) / 60;
    let rs = rem_secs % 60;

    PrayerSchedule {
        city: region.name.clone(),
        tz_label: region.tz_label.clone(),
        next_prayer_name: next_name.clone(),
        next_prayer_time: next_time.clone(),
        countdown_str: format!("-{:02}:{:02}:{:02}", rh, rm, rs),
        prayers,
    }
}
