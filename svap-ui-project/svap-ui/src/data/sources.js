export const ENFORCEMENT_SOURCES = [
  { id: "oig_newsroom", name: "HHS OIG Newsroom", url: "https://oig.hhs.gov/newsroom/", type: "press_releases", description: "Official OIG enforcement announcements, audit reports, and data briefs", frequency: "Weekly" },
  { id: "doj_healthcare", name: "DOJ Healthcare Fraud Unit", url: "https://www.justice.gov/criminal/criminal-fraud/health-care-fraud-unit", type: "press_releases", description: "DOJ Criminal Division healthcare fraud prosecutions and national takedowns", frequency: "Weekly" },
  { id: "doj_fca", name: "DOJ False Claims Act Settlements", url: "https://www.justice.gov/civil/fraud-statistics", type: "settlements", description: "Annual FCA statistics and major settlement announcements", frequency: "Annual" },
  { id: "oig_workplan", name: "OIG Work Plan", url: "https://oig.hhs.gov/reports/workplan/", type: "planned_work", description: "Active and planned OIG audits, evaluations, and investigations", frequency: "Ongoing" },
  { id: "oig_semiannual", name: "OIG Semiannual Report", url: "https://oig.hhs.gov/reports/semiannual/", type: "summary_report", description: "Comprehensive summary of OIG enforcement activity and recommendations", frequency: "Biannual" },
  { id: "cms_pi", name: "CMS Program Integrity", url: "https://www.cms.gov/About-CMS/Components/CPI", type: "program_data", description: "Improper payment rates, CERT findings, SMRC referrals", frequency: "Annual" },
  { id: "gao", name: "GAO Healthcare Reports", url: "https://www.gao.gov/topics/health", type: "audit_reports", description: "GAO audits including High Risk List updates for Medicare/Medicaid", frequency: "As published" },
  { id: "mfcu", name: "State MFCU Activity", url: "https://oig.hhs.gov/fraud/medicaid-fraud-control-units/", type: "state_enforcement", description: "Medicaid Fraud Control Unit statistical reports by state", frequency: "Annual" },
];

export const HHS_DATA_SOURCES = {
  claims: {
    label: "Claims & Encounters",
    sources: [
      { id: "cclf", name: "CMS Claims & Claims Line Feed (CCLF)", description: "Medicare FFS claims — Part A, B, D. Procedure codes, DX codes, NPI, dates, amounts." },
      { id: "taf", name: "Medicaid T-MSIS Analytic Files (TAF)", description: "Medicaid claims and enrollment. IP, OT, LT, RX claim types. ~6 month lag." },
      { id: "ma_encounter", name: "MA Encounter Data", description: "Medicare Advantage plan encounter submissions for risk adjustment validation." },
      { id: "pde", name: "Part D Prescription Drug Event (PDE)", description: "Part D drug claims. NDC, quantity, days supply, ingredient cost." },
    ],
  },
  enrollment: {
    label: "Enrollment & Eligibility",
    sources: [
      { id: "mbsf", name: "Medicare Beneficiary Summary File (MBSF)", description: "Enrollment, demographics, HCC risk scores, dual status." },
      { id: "taf_de", name: "TAF Demographic & Eligibility (DE)", description: "Medicaid enrollment, eligibility groups, managed care assignments." },
      { id: "ffe", name: "FFE Marketplace Enrollment", description: "ACA marketplace applications, plan selections, APTC amounts, broker IDs." },
    ],
  },
  provider: {
    label: "Provider & Supplier",
    sources: [
      { id: "pecos", name: "PECOS (Provider Enrollment)", description: "Enrollment dates, specialty, practice location, ownership." },
      { id: "nppes", name: "NPPES (NPI Registry)", description: "National provider identifiers, taxonomy codes, addresses." },
      { id: "leie", name: "LEIE (Exclusions Database)", description: "OIG exclusion actions. Excluded providers barred from federal programs." },
    ],
  },
  program_integrity: {
    label: "Program Integrity",
    sources: [
      { id: "cert", name: "CERT (Error Rate Testing)", description: "Improper payment findings by service type." },
      { id: "upic", name: "UPIC/SMRC Referrals", description: "Fraud referrals and investigation outcomes." },
      { id: "radv", name: "Risk Adjustment Data Validation", description: "MA risk adjustment audit findings, HCC error rates." },
      { id: "evv", name: "Electronic Visit Verification (EVV)", description: "Medicaid HCBS visit records. GPS, timestamps, attendant ID." },
    ],
  },
  financial: {
    label: "Financial & Payment",
    sources: [
      { id: "ma_bids", name: "MA Plan Bid Data", description: "Annual MA plan bids, projected costs, SSBCI projections." },
      { id: "340b", name: "340B OPAIS", description: "Covered entity registrations, contract pharmacy arrangements." },
      { id: "mdr", name: "Medicaid Drug Rebate", description: "Rebate unit amounts, utilization, best price reporting." },
      { id: "cost_reports", name: "Medicare Cost Reports", description: "Hospital/facility revenue, costs, utilization." },
    ],
  },
};
